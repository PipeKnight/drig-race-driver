package org.reassembly.grid;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.StringReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import org.junit.Rule;
import org.junit.Test;
import org.junit.rules.TemporaryFolder;

/** Desktop tests of the P10 import (gradle testDebugUnitTest, docs/ANDROID.md). */
public class GridImporterTest {
    @Rule public TemporaryFolder temp = new TemporaryFolder();

    /** A picked folder on the desktop; counts opened files. */
    private static class FileTree implements GridImporter.Tree {
        final File root;
        final List<String> opened = new ArrayList<>();

        FileTree(File root) {
            this.root = root;
        }

        private static GridImporter.Node node(File file) {
            return new GridImporter.Node(file.getPath(), file.getName(), file.isDirectory(),
                    file.isDirectory() ? -1 : file.length());
        }

        @Override
        public GridImporter.Node root() {
            return node(root);
        }

        @Override
        public List<GridImporter.Node> children(GridImporter.Node directory) {
            List<GridImporter.Node> nodes = new ArrayList<>();
            File[] files = new File(directory.id).listFiles();
            if (files != null) {
                for (File file : files) {
                    nodes.add(node(file));
                }
            }
            return nodes;
        }

        @Override
        public InputStream open(GridImporter.Node file) throws IOException {
            opened.add(file.name);
            return new FileInputStream(file.id);
        }
    }

    private static final GridImporter.Progress NO_PROGRESS = (done, total, path) -> {};
    private static final byte[] SETTINGS = bytes("[drive]\nenabled = on\n");

    /** The game files of the tests: manifest path to contents. */
    private final Map<String, byte[]> game = new TreeMap<>();

    {
        game.put("grid.exe", bytes("MZ not really an executable"));
        game.put("ActionMap/actionMapPreset4.xml", bytes("<preset/>"));
        game.put("tracks/europe/donington/route_0/route.xml", bytes("<route/>"));
    }

    private static byte[] bytes(String text) {
        return text.getBytes(StandardCharsets.UTF_8);
    }

    private static String sha(byte[] data) throws Exception {
        return GridImporter.hex(MessageDigest.getInstance("SHA-256").digest(data));
    }

    private GridImporter.Manifest manifest(String version) throws Exception {
        StringBuilder text = new StringBuilder("grid-import 1\nversion " + version + "\n");
        for (Map.Entry<String, byte[]> file : game.entrySet()) {
            text.append(file.getValue().length).append('\t').append(sha(file.getValue()))
                    .append('\t').append(file.getKey()).append('\n');
        }
        return GridImporter.parse(new StringReader(text.toString()));
    }

    /** Writes the game files under {@code folder}, optionally upper-cased paths. */
    private File copyGame(File folder, boolean upperCase) throws IOException {
        for (Map.Entry<String, byte[]> file : game.entrySet()) {
            String path = upperCase ? file.getKey().toUpperCase(java.util.Locale.ROOT) : file.getKey();
            File target = new File(folder, path);
            target.getParentFile().mkdirs();
            Files.write(target.toPath(), file.getValue());
        }
        return folder;
    }

    private static byte[] read(File file) throws IOException {
        return Files.readAllBytes(file.toPath());
    }

    @Test
    public void manifestRejectsBadLines() {
        String[] bad = {
            "grid-bundle 1\nversion v\n1\t" + "0".repeat(64) + "\ta\n",
            "grid-import 1\n1\t" + "0".repeat(64) + "\ta\n",
            "grid-import 1\nversion v\n1\tnothex\ta\n",
            "grid-import 1\nversion v\n-1\t" + "0".repeat(64) + "\ta\n",
            "grid-import 1\nversion v\n1\t" + "0".repeat(64) + "\t../a\n",
            "grid-import 1\nversion v\n1\t" + "0".repeat(64) + "\t/a\n",
            "grid-import 1\nversion v\n1\t" + "0".repeat(64) + "\ta\\b\n",
            "grid-import 1\nversion v\n",
        };
        for (String text : bad) {
            try {
                GridImporter.parse(new StringReader(text));
                fail("accepted: " + text);
            } catch (IOException expected) {
                // rejected
            }
        }
    }

    @Test
    public void findsTheGridFolderItselfOrInsideThePickedFolder() throws Exception {
        File direct = copyGame(temp.newFolder("direct"), false);
        assertEquals(direct.getPath(), GridImporter.locate(new FileTree(direct)).gridRoot.id);

        File parent = temp.newFolder("parent");
        new File(parent, "Other").mkdirs();
        File grid = copyGame(new File(parent, "GRID"), false);
        assertEquals(grid.getPath(), GridImporter.locate(new FileTree(parent)).gridRoot.id);

        assertNull(GridImporter.locate(new FileTree(temp.newFolder("empty"))));
    }

    @Test
    public void importCopiesWithTheManifestSpellingAndFinishes() throws Exception {
        GridImporter.Manifest manifest = manifest("v1");
        FileTree tree = new FileTree(copyGame(temp.newFolder("picked"), true));
        GridImporter.Resolver resolver = GridImporter.locate(tree);
        assertTrue(GridImporter.check(resolver, manifest).ok());
        File data = temp.newFolder("data");
        assertEquals(manifest.totalBytes(), GridImporter.remainingBytes(data, manifest));

        List<String> mismatches = GridImporter.copy(resolver, manifest, data, false, NO_PROGRESS);
        assertTrue(mismatches.isEmpty());
        for (Map.Entry<String, byte[]> file : game.entrySet()) {
            assertArrayEquals(file.getValue(), read(new File(data, "Grid/" + file.getKey())));
        }
        assertTrue(GridImporter.interrupted(data, manifest));
        assertEquals(0, GridImporter.remainingBytes(data, manifest));

        GridImporter.finish(data, manifest, SETTINGS);
        assertTrue(GridImporter.upToDate(data, manifest));
        assertFalse(GridImporter.interrupted(data, manifest));
        assertArrayEquals(SETTINGS, read(new File(data, "settings.ini")));
        assertFalse(GridImporter.upToDate(data, manifest("v2")));
    }

    @Test
    public void missingAndWrongSizeFilesAreReportedBeforeCopying() throws Exception {
        File picked = copyGame(temp.newFolder("picked"), false);
        assertTrue(new File(picked, "ActionMap/actionMapPreset4.xml").delete());
        Files.write(new File(picked, "tracks/europe/donington/route_0/route.xml").toPath(),
                bytes("<route>longer</route>"));
        GridImporter.Check check = GridImporter.check(
                GridImporter.locate(new FileTree(picked)), manifest("v1"));
        assertFalse(check.ok());
        assertEquals(Collections.singletonList("ActionMap/actionMapPreset4.xml"), check.missing);
        assertEquals(1, check.wrongSize.size());
        assertTrue(check.wrongSize.get(0).startsWith("tracks/europe/donington/route_0/route.xml"));
    }

    @Test
    public void anInterruptedImportResumesWithoutCopyingFinishedFilesAgain() throws Exception {
        GridImporter.Manifest manifest = manifest("v1");
        FileTree tree = new FileTree(copyGame(temp.newFolder("picked"), false));
        File data = temp.newFolder("data");
        // The first run is killed while copying the third file.
        try {
            GridImporter.copy(GridImporter.locate(tree), manifest, data, false,
                    (done, total, path) -> {
                        if (path.startsWith("tracks/")) {
                            throw new IllegalStateException("killed");
                        }
                    });
            fail("not interrupted");
        } catch (IllegalStateException expected) {
            // interrupted
        }
        assertTrue(GridImporter.interrupted(data, manifest));
        assertEquals(game.get("tracks/europe/donington/route_0/route.xml").length,
                GridImporter.remainingBytes(data, manifest));

        tree.opened.clear();
        assertTrue(GridImporter.copy(GridImporter.locate(tree), manifest, data, false, NO_PROGRESS)
                .isEmpty());
        assertEquals(Collections.singletonList("route.xml"), tree.opened);

        // A journalled file that was deleted or truncated is copied again.
        assertTrue(new File(data, "Grid/grid.exe").delete());
        tree.opened.clear();
        GridImporter.copy(GridImporter.locate(tree), manifest, data, false, NO_PROGRESS);
        assertEquals(Collections.singletonList("grid.exe"), tree.opened);
    }

    @Test
    public void otherContentsAreReportedAndCopiedOnlyWhenAccepted() throws Exception {
        GridImporter.Manifest manifest = manifest("v1");
        File picked = copyGame(temp.newFolder("picked"), false);
        byte[] other = bytes("MZ not really an EXECUTABLE");
        Files.write(new File(picked, "grid.exe").toPath(), other);
        File data = temp.newFolder("data");

        List<String> mismatches = GridImporter.copy(GridImporter.locate(new FileTree(picked)),
                manifest, data, false, NO_PROGRESS);
        assertEquals(Collections.singletonList("grid.exe"), mismatches);
        assertFalse(new File(data, "Grid/grid.exe").exists());
        assertFalse(new File(data, "Grid/grid.exe.part").exists());

        assertTrue(GridImporter.copy(GridImporter.locate(new FileTree(picked)), manifest, data, true,
                NO_PROGRESS).isEmpty());
        assertArrayEquals(other, read(new File(data, "Grid/grid.exe")));
        assertEquals(sha(other), GridImporter.journal(data).get("grid.exe"));
    }

    @Test
    public void finishKeepsAnEditedSettingsFileAndANewImportClearsTheMarker() throws Exception {
        GridImporter.Manifest manifest = manifest("v1");
        File data = temp.newFolder("data");
        Files.write(new File(data, "settings.ini").toPath(), bytes("[input]\ntilt = off\n"));
        FileTree tree = new FileTree(copyGame(temp.newFolder("picked"), false));
        GridImporter.copy(GridImporter.locate(tree), manifest, data, false, NO_PROGRESS);
        GridImporter.finish(data, manifest, SETTINGS);
        assertArrayEquals(bytes("[input]\ntilt = off\n"), read(new File(data, "settings.ini")));
        assertTrue(GridImporter.upToDate(data, manifest));

        GridImporter.copy(GridImporter.locate(tree), manifest, data, false, NO_PROGRESS);
        assertNull(GridImporter.installedVersion(data));
    }

    private static GridImporter.Manifest committed() throws Exception {
        File asset = new File("src/main/assets/grid-import/import.txt");
        try (java.io.Reader reader = new java.io.InputStreamReader(
                new FileInputStream(asset), StandardCharsets.UTF_8)) {
            return GridImporter.parse(reader);
        }
    }

    /**
     * The owner's installation imports cleanly with the committed manifest:
     * every file found, sizes and hashes equal. Runs only with GRID_GAME_ROOT
     * set (like the Rust asset tests); copies about 250 MB to a temp folder.
     */
    @Test
    public void theInstallationImportsWithTheCommittedManifest() throws Exception {
        String root = System.getenv("GRID_GAME_ROOT");
        org.junit.Assume.assumeTrue("GRID_GAME_ROOT not set", root != null && !root.isEmpty());
        File install = new File(root).getAbsoluteFile();
        GridImporter.Manifest manifest = committed();
        // The picked folder is the parent: the Grid folder is found inside it.
        GridImporter.Resolver resolver = GridImporter.locate(
                new FileTree(install.getParentFile()) {
                    @Override
                    public List<GridImporter.Node> children(GridImporter.Node directory) {
                        List<GridImporter.Node> all = super.children(directory);
                        if (!directory.id.equals(install.getParentFile().getPath())) {
                            return all;
                        }
                        // Only the installation itself, not every Steam game.
                        all.removeIf(n -> !n.id.equals(install.getPath()));
                        return all;
                    }
                });
        assertNotNull(resolver);
        assertTrue(GridImporter.check(resolver, manifest).ok());
        File data = temp.newFolder("data");
        assertTrue(GridImporter.copy(resolver, manifest, data, false, NO_PROGRESS).isEmpty());
        GridImporter.finish(data, manifest, SETTINGS);
        assertTrue(GridImporter.upToDate(data, manifest));
    }

    @Test
    public void theCommittedManifestParses() throws Exception {
        GridImporter.Manifest manifest = committed();
        assertEquals(64, manifest.version.length());
        boolean exe = false;
        for (GridImporter.Entry entry : manifest.entries) {
            exe |= entry.path.equals("grid.exe");
        }
        assertTrue(exe);
        assertNotNull(manifest.entries.get(0).sha256);
    }
}
