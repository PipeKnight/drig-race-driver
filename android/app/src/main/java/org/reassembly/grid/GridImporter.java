package org.reassembly.grid;

import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Imports the user's own GRID files into the app's data folder (P10,
 * docs/ANDROID.md "Importing the game data"), for the code-only APK, which
 * carries no game data. The user picks a GRID folder (copied from their own
 * installation); this class finds the files listed in the committed manifest
 * {@code assets/grid-import/import.txt} (sizes and SHA-256 only), checks
 * them, and copies them to {@code Grid/} in the data folder, the layout
 * tools/push_android_data.ps1 and the bundled APK write.
 *
 * <p>Resumable: each copied and verified file is appended to a journal, so a
 * new run skips it. The marker is written only after the whole import (and
 * the native check) succeeded.
 *
 * <p>Plain Java without Android classes, so the desktop unit tests
 * (app/src/test) run it against folders.
 */
final class GridImporter {
    static final String ASSET_ROOT = "grid-import";
    static final String MANIFEST = "import.txt";
    static final String SETTINGS = "settings.ini";
    /** In the data folder: the manifest version of the last complete import. */
    static final String MARKER = ".import-version";
    /** In the data folder: "sha256\tpath" per file copied so far. */
    static final String JOURNAL = ".import-progress";
    static final String HEADER = "grid-import 1";
    static final String GRID = "Grid";
    static final String EXE = "grid.exe";

    /** One file or folder of the picked tree. */
    static final class Node {
        final String id;
        final String name;
        final boolean directory;
        final long size;

        Node(String id, String name, boolean directory, long size) {
            this.id = id;
            this.name = name;
            this.directory = directory;
            this.size = size;
        }
    }

    /** The folder the user picked (Storage Access Framework on the phone). */
    interface Tree {
        Node root() throws IOException;

        List<Node> children(Node directory) throws IOException;

        InputStream open(Node file) throws IOException;
    }

    interface Progress {
        void update(long doneBytes, long totalBytes, String path);
    }

    static final class Entry {
        final String path;
        final long size;
        final String sha256;

        Entry(String path, long size, String sha256) {
            this.path = path;
            this.size = size;
            this.sha256 = sha256;
        }
    }

    static final class Manifest {
        final String version;
        final List<Entry> entries;

        Manifest(String version, List<Entry> entries) {
            this.version = version;
            this.entries = Collections.unmodifiableList(entries);
        }

        long totalBytes() {
            long total = 0;
            for (Entry entry : entries) {
                total += entry.size;
            }
            return total;
        }
    }

    private GridImporter() {}

    /**
     * Reads import.txt:
     * <pre>
     * grid-import 1
     * version &lt;hex&gt;
     * &lt;size&gt;\t&lt;sha256&gt;\t&lt;path relative to the GRID folder&gt;
     * </pre>
     */
    static Manifest parse(Reader reader) throws IOException {
        BufferedReader lines = new BufferedReader(reader);
        String header = lines.readLine();
        if (header != null && header.startsWith("﻿")) {
            header = header.substring(1);
        }
        if (!HEADER.equals(header)) {
            throw new IOException("import.txt: unknown header " + header);
        }
        String versionLine = lines.readLine();
        if (versionLine == null || !versionLine.startsWith("version ")
                || versionLine.substring(8).trim().isEmpty()) {
            throw new IOException("import.txt: missing version line");
        }
        String version = versionLine.substring(8).trim();
        List<Entry> entries = new ArrayList<>();
        String line;
        int number = 2;
        while ((line = lines.readLine()) != null) {
            number++;
            if (line.isEmpty()) {
                continue;
            }
            String[] fields = line.split("\t", 3);
            if (fields.length != 3) {
                throw new IOException("import.txt line " + number + ": expected 3 fields");
            }
            long size;
            try {
                size = Long.parseLong(fields[0]);
            } catch (NumberFormatException e) {
                throw new IOException("import.txt line " + number + ": bad size " + fields[0]);
            }
            String sha = fields[1].toLowerCase(Locale.ROOT);
            if (size < 0 || !sha.matches("[0-9a-f]{64}")) {
                throw new IOException("import.txt line " + number + ": bad size or hash");
            }
            checkPath(fields[2], number);
            entries.add(new Entry(fields[2], size, sha));
        }
        if (entries.isEmpty()) {
            throw new IOException("import.txt: no files");
        }
        return new Manifest(version, entries);
    }

    /** Relative, '/'-separated, no empty, "." or ".." segment: stays inside Grid/. */
    private static void checkPath(String path, int number) throws IOException {
        boolean bad = path.isEmpty() || path.startsWith("/") || path.indexOf('\\') >= 0
                || path.indexOf(':') >= 0;
        if (!bad) {
            for (String segment : path.split("/", -1)) {
                if (segment.isEmpty() || segment.equals(".") || segment.equals("..")) {
                    bad = true;
                    break;
                }
            }
        }
        if (bad) {
            throw new IOException("import.txt line " + number + ": bad path " + path);
        }
    }

    /**
     * Looks files up by path, ignoring case (a folder copied from Windows may
     * differ in case; the copy keeps the manifest's spelling, which the game
     * expects). Folder listings are cached.
     */
    static final class Resolver {
        private final Tree tree;
        final Node gridRoot;
        private final Map<String, Map<String, Node>> listings = new HashMap<>();

        Resolver(Tree tree, Node gridRoot) {
            this.tree = tree;
            this.gridRoot = gridRoot;
        }

        private Map<String, Node> listing(Node directory) throws IOException {
            Map<String, Node> names = listings.get(directory.id);
            if (names == null) {
                names = new HashMap<>();
                for (Node child : tree.children(directory)) {
                    names.put(child.name.toLowerCase(Locale.ROOT), child);
                }
                listings.put(directory.id, names);
            }
            return names;
        }

        /** The file at {@code path} below the GRID folder, or null. */
        Node find(String path) throws IOException {
            Node node = gridRoot;
            String[] segments = path.split("/");
            for (int i = 0; i < segments.length; i++) {
                if (!node.directory) {
                    return null;
                }
                node = listing(node).get(segments[i].toLowerCase(Locale.ROOT));
                if (node == null) {
                    return null;
                }
            }
            return node.directory ? null : node;
        }

        InputStream open(Node file) throws IOException {
            return tree.open(file);
        }
    }

    /**
     * The GRID folder: the picked folder when it holds grid.exe, else a
     * sub-folder that does ("Grid" first). Null when there is none.
     */
    static Resolver locate(Tree tree) throws IOException {
        Node root = tree.root();
        Resolver direct = new Resolver(tree, root);
        if (direct.find(EXE) != null) {
            return direct;
        }
        List<Node> folders = new ArrayList<>();
        for (Node child : tree.children(root)) {
            if (child.directory) {
                folders.add(child);
            }
        }
        // "Grid" (any case) before other folders.
        folders.sort((a, b) -> Boolean.compare(
                !a.name.equalsIgnoreCase(GRID), !b.name.equalsIgnoreCase(GRID)));
        for (Node folder : folders) {
            Resolver inner = new Resolver(tree, folder);
            if (inner.find(EXE) != null) {
                return inner;
            }
        }
        return null;
    }

    /** What the picked folder lacks; empty lists mean the copy can start. */
    static final class Check {
        final List<String> missing = new ArrayList<>();
        /** "path (found N bytes, need M)". */
        final List<String> wrongSize = new ArrayList<>();

        boolean ok() {
            return missing.isEmpty() && wrongSize.isEmpty();
        }
    }

    static Check check(Resolver resolver, Manifest manifest) throws IOException {
        Check check = new Check();
        for (Entry entry : manifest.entries) {
            Node node = resolver.find(entry.path);
            if (node == null) {
                check.missing.add(entry.path);
            } else if (node.size >= 0 && node.size != entry.size) {
                check.wrongSize.add(entry.path + " (found " + node.size + " bytes, need "
                        + entry.size + ")");
            }
        }
        return check;
    }

    /** The version of the last complete import, or null. */
    static String installedVersion(File dataDir) {
        File marker = new File(dataDir, MARKER);
        if (!marker.isFile()) {
            return null;
        }
        try (BufferedReader in = new BufferedReader(new InputStreamReader(
                new FileInputStream(marker), StandardCharsets.UTF_8))) {
            String line = in.readLine();
            return line == null ? null : line.trim();
        } catch (IOException e) {
            return null;
        }
    }

    static boolean upToDate(File dataDir, Manifest manifest) {
        return manifest.version.equals(installedVersion(dataDir));
    }

    /** An import was started and not finished (the launcher offers to continue). */
    static boolean interrupted(File dataDir, Manifest manifest) {
        return new File(dataDir, JOURNAL).isFile() && !upToDate(dataDir, manifest);
    }

    /** Files already copied and verified: path to the SHA-256 they had. */
    static Map<String, String> journal(File dataDir) {
        Map<String, String> done = new HashMap<>();
        File file = new File(dataDir, JOURNAL);
        if (!file.isFile()) {
            return done;
        }
        try (BufferedReader in = new BufferedReader(new InputStreamReader(
                new FileInputStream(file), StandardCharsets.UTF_8))) {
            String line;
            while ((line = in.readLine()) != null) {
                String[] fields = line.split("\t", 2);
                // A line cut off by a kill has no tab or a short hash: ignored.
                if (fields.length == 2 && fields[0].length() == 64) {
                    done.put(fields[1], fields[0]);
                }
            }
        } catch (IOException e) {
            // An unreadable journal means copying everything again.
        }
        return done;
    }

    /** Bytes still to copy (for the free-space check). */
    static long remainingBytes(File dataDir, Manifest manifest) {
        Map<String, String> done = journal(dataDir);
        long remaining = 0;
        for (Entry entry : manifest.entries) {
            if (!copied(dataDir, entry, done, true)) {
                remaining += entry.size;
            }
        }
        return remaining;
    }

    private static boolean copied(File dataDir, Entry entry, Map<String, String> done,
            boolean acceptMismatches) {
        String sha = done.get(entry.path);
        File target = new File(new File(dataDir, GRID), entry.path);
        return sha != null && (acceptMismatches || sha.equals(entry.sha256))
                && target.isFile() && target.length() == entry.size;
    }

    /**
     * Copies every manifest file not yet in the journal, checking its
     * SHA-256 on the way. Returns the paths whose content differs from the
     * manifest (not copied) unless {@code acceptMismatches}; an empty list
     * means every file is in place. Does not write the marker
     * ({@link #finish}).
     */
    static List<String> copy(Resolver resolver, Manifest manifest, File dataDir,
            boolean acceptMismatches, Progress progress) throws IOException {
        File grid = new File(dataDir, GRID);
        if (!grid.isDirectory() && !grid.mkdirs()) {
            throw new IOException("cannot create " + grid);
        }
        File marker = new File(dataDir, MARKER);
        if (marker.exists() && !marker.delete()) {
            throw new IOException("cannot delete " + marker);
        }
        Map<String, String> done = journal(dataDir);
        List<String> mismatches = new ArrayList<>();
        long total = manifest.totalBytes();
        long doneBytes = 0;
        byte[] buffer = new byte[1 << 16];
        progress.update(0, total, "");
        try (OutputStream journal = new FileOutputStream(new File(dataDir, JOURNAL), true)) {
            for (Entry entry : manifest.entries) {
                if (copied(dataDir, entry, done, acceptMismatches)) {
                    doneBytes += entry.size;
                    progress.update(doneBytes, total, entry.path);
                    continue;
                }
                Node node = resolver.find(entry.path);
                if (node == null) {
                    throw new IOException(entry.path + " is no longer in the picked folder");
                }
                File target = new File(grid, entry.path);
                File parent = target.getParentFile();
                if (parent != null && !parent.isDirectory() && !parent.mkdirs()) {
                    throw new IOException("cannot create " + parent);
                }
                File part = new File(target.getPath() + ".part");
                MessageDigest digest = sha256();
                long copiedBytes = 0;
                try (InputStream in = resolver.open(node);
                        OutputStream out = new FileOutputStream(part)) {
                    int read;
                    while ((read = in.read(buffer)) > 0) {
                        out.write(buffer, 0, read);
                        digest.update(buffer, 0, read);
                        copiedBytes += read;
                        if (copiedBytes > entry.size) {
                            break;
                        }
                        progress.update(doneBytes + copiedBytes, total, entry.path);
                    }
                }
                if (copiedBytes != entry.size) {
                    part.delete();
                    throw new IOException(entry.path + ": read " + copiedBytes
                            + " bytes, expected " + entry.size);
                }
                String sha = hex(digest.digest());
                if (!sha.equals(entry.sha256) && !acceptMismatches) {
                    part.delete();
                    mismatches.add(entry.path);
                    doneBytes += entry.size;
                    continue;
                }
                replace(part, target);
                journal.write((sha + "\t" + entry.path + "\n").getBytes(StandardCharsets.UTF_8));
                journal.flush();
                done.put(entry.path, sha);
                doneBytes += entry.size;
            }
        }
        progress.update(total, total, "");
        return mismatches;
    }

    /**
     * After the copy and the native check: writes settings.ini when missing
     * (the owner's edits stay) and the marker.
     */
    static void finish(File dataDir, Manifest manifest, byte[] settings) throws IOException {
        File settingsFile = new File(dataDir, SETTINGS);
        if (!settingsFile.isFile()) {
            write(new File(dataDir, SETTINGS + ".part"), settings);
            replace(new File(dataDir, SETTINGS + ".part"), settingsFile);
        }
        File markerPart = new File(dataDir, MARKER + ".part");
        write(markerPart, (manifest.version + "\n").getBytes(StandardCharsets.UTF_8));
        replace(markerPart, new File(dataDir, MARKER));
    }

    private static void write(File file, byte[] bytes) throws IOException {
        try (OutputStream out = new FileOutputStream(file)) {
            out.write(bytes);
        }
    }

    private static MessageDigest sha256() {
        try {
            return MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException(e);
        }
    }

    static String hex(byte[] bytes) {
        StringBuilder text = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            text.append(String.format(Locale.ROOT, "%02x", b));
        }
        return text.toString();
    }

    private static void replace(File from, File to) throws IOException {
        // File.renameTo replaces on Android (rename(2)) but not on Windows,
        // where the desktop unit tests run.
        if (to.exists() && !to.delete()) {
            throw new IOException("cannot replace " + to);
        }
        if (!from.renameTo(to)) {
            throw new IOException("cannot rename " + from + " to " + to);
        }
    }
}
