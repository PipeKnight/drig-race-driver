package org.reassembly.grid;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.io.StringReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.junit.Rule;
import org.junit.Test;
import org.junit.rules.TemporaryFolder;

/** Desktop tests of the first-run copy (gradle testDebugUnitTest, docs/ANDROID.md). */
public class BundleExtractorTest {
    @Rule public TemporaryFolder temp = new TemporaryFolder();

    private final Map<String, byte[]> assets = new HashMap<>();
    private final List<String> opened = new ArrayList<>();

    private final BundleExtractor.Source source = path -> {
        opened.add(path);
        byte[] bytes = assets.get(path);
        if (bytes == null) {
            throw new FileNotFoundException(path);
        }
        return new ByteArrayInputStream(bytes);
    };

    private static final BundleExtractor.Progress NO_PROGRESS = (done, total) -> {};

    private String manifest(String version) {
        StringBuilder text = new StringBuilder("grid-bundle 1\nversion " + version + "\n");
        for (Map.Entry<String, byte[]> asset : new java.util.TreeMap<>(assets).entrySet()) {
            String mode = asset.getKey().equals("settings.ini") ? "keep" : "replace";
            text.append(asset.getValue().length).append('\t').append(mode).append('\t')
                    .append(asset.getKey()).append('\n');
        }
        return text.toString();
    }

    private static BundleExtractor.Manifest parse(String text) throws IOException {
        return BundleExtractor.parse(new StringReader(text));
    }

    private static byte[] bytes(String text) {
        return text.getBytes(StandardCharsets.UTF_8);
    }

    private void standardAssets() {
        assets.put("settings.ini", bytes("[drive]\nenabled = on\n"));
        assets.put("Grid/grid.exe", bytes("MZ not really"));
        assets.put("Grid/tracks/europe/donington/route_0/track.jpk", new byte[200_000]);
        assets.put("Grid/audio/CMSamples.dic", bytes("dic"));
    }

    @Test
    public void firstRunCopiesEverythingAndWritesTheMarker() throws Exception {
        standardAssets();
        File data = temp.newFolder("files");
        BundleExtractor.Manifest m = parse(manifest("abc123"));
        assertFalse(BundleExtractor.upToDate(data, m));
        List<long[]> updates = new ArrayList<>();
        BundleExtractor.extract(source, m, data, (done, total) -> updates.add(new long[] {done, total}));
        for (Map.Entry<String, byte[]> asset : assets.entrySet()) {
            assertArrayEquals(asset.getKey(), asset.getValue(),
                    Files.readAllBytes(new File(data, asset.getKey()).toPath()));
            assertFalse(new File(data, asset.getKey() + ".part").exists());
        }
        assertEquals("abc123", BundleExtractor.installedVersion(data));
        assertTrue(BundleExtractor.upToDate(data, m));
        long total = m.totalBytes();
        assertEquals(200_000 + 13 + 3 + 21, total);
        long previous = -1;
        for (long[] update : updates) {
            assertEquals(total, update[1]);
            assertTrue("progress never goes back", update[0] >= previous);
            previous = update[0];
        }
        assertEquals(total, previous);
    }

    @Test
    public void sameVersionIsUpToDateAndNewVersionReplacesFilesButKeepsSettings() throws Exception {
        standardAssets();
        File data = temp.newFolder("files");
        BundleExtractor.extract(source, parse(manifest("v1")), data, NO_PROGRESS);
        File settings = new File(data, "settings.ini");
        Files.write(settings.toPath(), bytes("[drive]\nenabled = off\n"));

        assertTrue(BundleExtractor.upToDate(data, parse(manifest("v1"))));

        assets.put("Grid/grid.exe", bytes("MZ version two"));
        BundleExtractor.Manifest v2 = parse(manifest("v2"));
        assertFalse(BundleExtractor.upToDate(data, v2));
        opened.clear();
        BundleExtractor.extract(source, v2, data, NO_PROGRESS);
        assertArrayEquals(bytes("MZ version two"),
                Files.readAllBytes(new File(data, "Grid/grid.exe").toPath()));
        assertArrayEquals("the owner's settings stay", bytes("[drive]\nenabled = off\n"),
                Files.readAllBytes(settings.toPath()));
        assertFalse(opened.contains("settings.ini"));
        assertEquals("v2", BundleExtractor.installedVersion(data));
    }

    @Test
    public void missingSettingsAreWrittenAgain() throws Exception {
        standardAssets();
        File data = temp.newFolder("files");
        BundleExtractor.extract(source, parse(manifest("v1")), data, NO_PROGRESS);
        assertTrue(new File(data, "settings.ini").delete());
        BundleExtractor.extract(source, parse(manifest("v2")), data, NO_PROGRESS);
        assertArrayEquals(assets.get("settings.ini"),
                Files.readAllBytes(new File(data, "settings.ini").toPath()));
    }

    @Test
    public void interruptedCopyLeavesNoMarkerSoItRunsAgain() throws Exception {
        standardAssets();
        File data = temp.newFolder("files");
        BundleExtractor.extract(source, parse(manifest("v1")), data, NO_PROGRESS);
        String text = manifest("v2");
        assets.remove("Grid/tracks/europe/donington/route_0/track.jpk");
        try {
            BundleExtractor.extract(source, parse(text), data, NO_PROGRESS);
            fail("a missing asset must fail");
        } catch (FileNotFoundException expected) {
            // The old marker is gone, so the next start copies again.
        }
        assertNull(BundleExtractor.installedVersion(data));
        assertFalse(BundleExtractor.upToDate(data, parse(text)));
    }

    @Test
    public void sizeMismatchFailsWithoutReplacingTheFile() throws Exception {
        assets.put("Grid/a.bin", bytes("12345"));
        File data = temp.newFolder("files");
        Files.createDirectories(new File(data, "Grid").toPath());
        Files.write(new File(data, "Grid/a.bin").toPath(), bytes("old"));
        BundleExtractor.Manifest wrong = parse("grid-bundle 1\nversion v\n4\treplace\tGrid/a.bin\n");
        try {
            BundleExtractor.extract(source, wrong, data, NO_PROGRESS);
            fail("size mismatch must fail");
        } catch (IOException expected) {
            assertTrue(expected.getMessage(), expected.getMessage().contains("Grid/a.bin"));
        }
        assertArrayEquals(bytes("old"), Files.readAllBytes(new File(data, "Grid/a.bin").toPath()));
        assertFalse(new File(data, "Grid/a.bin.part").exists());
        assertNull(BundleExtractor.installedVersion(data));
    }

    @Test
    public void parseRejectsPathsLeavingTheDataFolderAndBadLines() {
        String[] bad = {
            "",
            "grid-bundle 2\nversion v\n1\treplace\ta\n",
            "grid-bundle 1\n1\treplace\ta\n",
            "grid-bundle 1\nversion v\n",
            "grid-bundle 1\nversion v\n1\treplace\t../escape\n",
            "grid-bundle 1\nversion v\n1\treplace\tGrid/../../escape\n",
            "grid-bundle 1\nversion v\n1\treplace\t/abs\n",
            "grid-bundle 1\nversion v\n1\treplace\tC:/abs\n",
            "grid-bundle 1\nversion v\n1\treplace\tGrid\\win\n",
            "grid-bundle 1\nversion v\n1\treplace\tGrid//double\n",
            "grid-bundle 1\nversion v\n1\treplace\t.bundle-version\n",
            "grid-bundle 1\nversion v\nx\treplace\ta\n",
            "grid-bundle 1\nversion v\n-1\treplace\ta\n",
            "grid-bundle 1\nversion v\n1\tmaybe\ta\n",
            "grid-bundle 1\nversion v\n1\treplace\n",
        };
        for (String text : bad) {
            try {
                parse(text);
                fail("accepted: " + text);
            } catch (IOException expected) {
                // rejected as it should be
            }
        }
    }

    @Test
    public void parseAcceptsByteOrderMarkSpacesAndBlankLines() throws Exception {
        BundleExtractor.Manifest m = parse(
                "﻿grid-bundle 1\nversion  ff00 \n\n3\tkeep\tsettings.ini\n7\treplace\tGrid/system/a b.xml\n");
        assertEquals("ff00", m.version);
        assertEquals(2, m.entries.size());
        assertTrue(m.entries.get(0).keep);
        assertEquals("Grid/system/a b.xml", m.entries.get(1).path);
        assertEquals(10, m.totalBytes());
    }
}
