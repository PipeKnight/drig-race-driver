package org.reassembly.grid;

import java.io.BufferedReader;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Copies the game data packed into a private APK (tools/build_android.ps1
 * -BundleData, docs/ANDROID.md) into the app's data folder, in the layout
 * tools/push_android_data.ps1 writes: {@code settings.ini} and {@code Grid/...}.
 *
 * <p>The APK assets hold {@code grid-data/bundle.txt} and the files it lists
 * under {@code grid-data/}. A marker file in the data folder records the
 * bundle version (a SHA-256 written by the build script), so the copy runs on
 * the first start and again only when the bundled data changes.
 *
 * <p>Plain Java without Android classes, so the desktop unit tests
 * (app/src/test) run it against a folder.
 */
final class BundleExtractor {
    /** Asset folder of the bundle inside the APK. */
    static final String ASSET_ROOT = "grid-data";
    static final String MANIFEST = "bundle.txt";
    /** In the data folder; holds the version of the last complete copy. */
    static final String MARKER = ".bundle-version";
    /** In the data folder; locked while a copy runs (any process). */
    static final String LOCK = ".bundle-lock";
    static final String HEADER = "grid-bundle 1";

    /** Opens one bundled file; {@code path} is relative to {@link #ASSET_ROOT}. */
    interface Source {
        InputStream open(String path) throws IOException;
    }

    interface Progress {
        void update(long doneBytes, long totalBytes);
    }

    static final class Entry {
        final String path;
        final long size;
        /** {@code keep}: written only when missing (settings.ini the owner may edit). */
        final boolean keep;

        Entry(String path, long size, boolean keep) {
            this.path = path;
            this.size = size;
            this.keep = keep;
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

    private BundleExtractor() {}

    /**
     * Reads bundle.txt:
     * <pre>
     * grid-bundle 1
     * version &lt;hex&gt;
     * &lt;size&gt;\t&lt;replace|keep&gt;\t&lt;relative/path&gt;
     * </pre>
     */
    static Manifest parse(Reader reader) throws IOException {
        BufferedReader lines = new BufferedReader(reader);
        String header = lines.readLine();
        if (!HEADER.equals(stripBom(header))) {
            throw new IOException("bundle.txt: unknown header " + header);
        }
        String versionLine = lines.readLine();
        if (versionLine == null || !versionLine.startsWith("version ")) {
            throw new IOException("bundle.txt: missing version line");
        }
        String version = versionLine.substring("version ".length()).trim();
        if (version.isEmpty()) {
            throw new IOException("bundle.txt: empty version");
        }
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
                throw new IOException("bundle.txt line " + number + ": expected 3 fields");
            }
            long size;
            try {
                size = Long.parseLong(fields[0]);
            } catch (NumberFormatException e) {
                throw new IOException("bundle.txt line " + number + ": bad size " + fields[0]);
            }
            if (size < 0) {
                throw new IOException("bundle.txt line " + number + ": negative size");
            }
            boolean keep;
            if (fields[1].equals("keep")) {
                keep = true;
            } else if (fields[1].equals("replace")) {
                keep = false;
            } else {
                throw new IOException("bundle.txt line " + number + ": bad mode " + fields[1]);
            }
            checkPath(fields[2], number);
            entries.add(new Entry(fields[2], size, keep));
        }
        if (entries.isEmpty()) {
            throw new IOException("bundle.txt: no files");
        }
        return new Manifest(version, entries);
    }

    /** Relative, '/'-separated, no empty, "." or ".." segment: stays inside the data folder. */
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
        if (bad || path.equals(MARKER)) {
            throw new IOException("bundle.txt line " + number + ": bad path " + path);
        }
    }

    private static String stripBom(String text) {
        return text != null && text.startsWith("﻿") ? text.substring(1) : text;
    }

    /** The APK's bundle.txt, or null when the APK has no bundled data. */
    static Manifest bundled(android.content.Context context) throws IOException {
        InputStream in;
        try {
            in = context.getAssets().open(ASSET_ROOT + "/" + MANIFEST);
        } catch (java.io.FileNotFoundException e) {
            return null;
        }
        try (java.io.Reader reader = new java.io.InputStreamReader(in, StandardCharsets.UTF_8)) {
            return parse(reader);
        }
    }

    /** The version of the last complete copy, or null. */
    static String installedVersion(File dataDir) {
        File marker = new File(dataDir, MARKER);
        if (!marker.isFile()) {
            return null;
        }
        try (InputStream in = new java.io.FileInputStream(marker)) {
            byte[] bytes = new byte[256];
            int length = 0;
            int read;
            while (length < bytes.length && (read = in.read(bytes, length, bytes.length - length)) > 0) {
                length += read;
            }
            return new String(bytes, 0, length, StandardCharsets.UTF_8).trim();
        } catch (IOException e) {
            return null;
        }
    }

    static boolean upToDate(File dataDir, Manifest manifest) {
        return manifest.version.equals(installedVersion(dataDir));
    }

    /**
     * Copies every entry, then writes the marker. The marker is removed first,
     * so an interrupted copy (app killed, storage full) runs again next start.
     * Each file goes to "name.part" and is renamed when its size matches.
     */
    static void extract(Source source, Manifest manifest, File dataDir, Progress progress)
            throws IOException {
        if (!dataDir.isDirectory() && !dataDir.mkdirs()) {
            throw new IOException("cannot create " + dataDir);
        }
        // One copy at a time across processes (F451): each event-menu
        // relaunch is a new process, and two copies replacing the same files
        // made files vanish under a running race. The lock ends with the
        // process; a copy that finished meanwhile is not repeated.
        try (java.io.RandomAccessFile lockFile = new java.io.RandomAccessFile(new File(dataDir, LOCK), "rw");
                java.nio.channels.FileLock lock = lockFile.getChannel().lock()) {
            if (upToDate(dataDir, manifest)) {
                progress.update(manifest.totalBytes(), manifest.totalBytes());
                return;
            }
            copyAll(source, manifest, dataDir, progress);
        }
    }

    private static void copyAll(Source source, Manifest manifest, File dataDir, Progress progress)
            throws IOException {
        File marker = new File(dataDir, MARKER);
        if (marker.exists() && !marker.delete()) {
            throw new IOException("cannot delete " + marker);
        }
        long total = manifest.totalBytes();
        long done = 0;
        byte[] buffer = new byte[1 << 16];
        progress.update(0, total);
        for (Entry entry : manifest.entries) {
            File target = new File(dataDir, entry.path);
            if (entry.keep && target.isFile()) {
                done += entry.size;
                progress.update(done, total);
                continue;
            }
            File parent = target.getParentFile();
            if (parent != null && !parent.isDirectory() && !parent.mkdirs()) {
                throw new IOException("cannot create " + parent);
            }
            File part = new File(target.getPath() + ".part");
            long copied = 0;
            try (InputStream in = source.open(entry.path);
                    OutputStream out = new FileOutputStream(part)) {
                int read;
                while ((read = in.read(buffer)) > 0) {
                    out.write(buffer, 0, read);
                    copied += read;
                    if (copied > entry.size) {
                        break;
                    }
                    progress.update(done + copied, total);
                }
            }
            if (copied != entry.size) {
                part.delete();
                throw new IOException(entry.path + ": bundled " + copied + " bytes, bundle.txt says "
                        + entry.size);
            }
            replace(part, target);
            done += entry.size;
        }
        File markerPart = new File(dataDir, MARKER + ".part");
        try (OutputStream out = new FileOutputStream(markerPart)) {
            out.write((manifest.version + "\n").getBytes(StandardCharsets.UTF_8));
        }
        replace(markerPart, marker);
        progress.update(total, total);
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
