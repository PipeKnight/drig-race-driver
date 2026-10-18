package org.reassembly.grid;

import android.app.Activity;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.UriPermission;
import android.graphics.Color;
import android.net.Uri;
import android.provider.DocumentsContract;
import android.util.Log;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.View;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.ScrollView;
import android.widget.TextView;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.PrintWriter;
import java.io.Reader;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Locale;

/**
 * The code-only APK's import screens (P10, docs/ANDROID.md "Importing the
 * game data"): explains what is needed, opens the system folder picker
 * (ACTION_OPEN_DOCUMENT_TREE), checks and copies the files with
 * {@link GridImporter}, runs {@link NativeCheck}, then starts the game.
 * Plain Android views: touch works, and a pad moves the focus with the
 * D-pad or stick, A presses the focused button and B goes back.
 */
final class ImportFlow {
    private static final String TAG = "grid";
    private static final int PICK = 41;
    private static final String PREFS = "import";
    private static final String TREE = "tree";
    /** Free space kept beyond the files themselves. */
    private static final long SPARE_BYTES = 64L << 20;

    /** One import per process, even if the activity is recreated meanwhile. */
    private static Thread worker;
    private static LauncherActivity shown;
    private static GridImporter.Manifest manifest;
    private static File dataDir;

    private static ProgressBar bar;
    private static TextView status;

    private ImportFlow() {}

    static void start(LauncherActivity activity, File folder) {
        dataDir = folder;
        try {
            manifest = readManifest(activity);
        } catch (Exception e) {
            problem(activity, "Cannot read the import file list of this APK.", e, false);
            return;
        }
        if (dataDir == null) {
            problem(activity, "The app's storage folder is not available (shared storage not mounted?).",
                    null, false);
            return;
        }
        if (worker == null && GridImporter.upToDate(dataDir, manifest)) {
            activity.startGame();
            return;
        }
        // Data placed by adb (tools/push_android_data.ps1) or left by an
        // earlier bundled APK: the development path, unchanged.
        if (worker == null && !GridImporter.interrupted(dataDir, manifest)
                && new File(dataDir, GridImporter.GRID + "/" + GridImporter.EXE).isFile()) {
            Log.i(TAG, "import: game data already present (adb or bundled), starting");
            activity.startGame();
            return;
        }
        synchronized (ImportFlow.class) {
            shown = activity;
        }
        if (worker != null) {
            progressScreen(activity, "Copying the game data");
        } else if (GridImporter.interrupted(dataDir, manifest) && savedTree(activity) != null) {
            resumeScreen(activity);
        } else {
            welcome(activity);
        }
    }

    static void detach(LauncherActivity activity) {
        synchronized (ImportFlow.class) {
            if (shown == activity) {
                shown = null;
            }
        }
    }

    private static GridImporter.Manifest readManifest(Activity activity) throws Exception {
        try (Reader reader = new InputStreamReader(activity.getAssets().open(
                GridImporter.ASSET_ROOT + "/" + GridImporter.MANIFEST), StandardCharsets.UTF_8)) {
            return GridImporter.parse(reader);
        }
    }

    // ---- Screens --------------------------------------------------------

    private static int dp(Activity activity, int value) {
        return (int) (value * activity.getResources().getDisplayMetrics().density);
    }

    private static LinearLayout column(Activity activity, String titleText) {
        LinearLayout column = new LinearLayout(activity);
        column.setOrientation(LinearLayout.VERTICAL);
        column.setGravity(Gravity.CENTER_HORIZONTAL);
        column.setBackgroundColor(Color.BLACK);
        int pad = dp(activity, 24);
        column.setPadding(pad, pad, pad, pad);
        TextView title = new TextView(activity);
        title.setText(titleText);
        title.setTextColor(Color.WHITE);
        title.setTextSize(22);
        title.setGravity(Gravity.CENTER);
        column.addView(title);
        return column;
    }

    private static TextView text(Activity activity, LinearLayout column, String body) {
        TextView view = new TextView(activity);
        view.setText(body);
        view.setTextColor(Color.LTGRAY);
        view.setTextSize(16);
        int pad = dp(activity, 12);
        view.setPadding(0, pad, 0, pad);
        column.addView(view);
        return view;
    }

    private static Button button(Activity activity, LinearLayout row, String label,
            View.OnClickListener click) {
        Button button = new Button(activity);
        button.setText(label);
        button.setAllCaps(false);
        button.setFocusable(true);
        button.setOnClickListener(click);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        params.setMargins(dp(activity, 8), dp(activity, 4), dp(activity, 8), dp(activity, 4));
        row.addView(button, params);
        return button;
    }

    private static LinearLayout row(Activity activity, LinearLayout column) {
        LinearLayout row = new LinearLayout(activity);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER);
        column.addView(row);
        return row;
    }

    private static void show(Activity activity, LinearLayout column, View focus) {
        bar = null;
        status = null;
        ScrollView scroll = new ScrollView(activity);
        scroll.setBackgroundColor(Color.BLACK);
        scroll.setFillViewport(true);
        scroll.addView(column);
        activity.setContentView(scroll);
        if (focus != null) {
            focus.requestFocus();
        }
    }

    private static String megabytes(long bytes) {
        return String.format(Locale.ROOT, "%d MB", (bytes + (1 << 19)) >> 20);
    }

    private static void welcome(LauncherActivity activity) {
        LinearLayout column = column(activity, "GRID Reassembly: your game files are needed");
        text(activity, column, "This app is an independent engine for Race Driver: GRID (2008) and "
                + "carries no game data. It uses the files of your own copy of the game.\n\n"
                + "1. On your PC, copy the GRID folder of your installation (Steam: "
                + "steamapps\\common\\Grid) to this phone, for example over USB into "
                + "Documents/GRID. The whole folder works; the app reads only what it needs.\n"
                + "2. Tap \"Choose GRID folder\" and pick that folder (the one with grid.exe, "
                + "or the folder above it).\n"
                + "3. The app checks " + manifest.entries.size() + " files ("
                + megabytes(manifest.totalBytes()) + ") and copies them into its own storage. "
                + "Afterwards the copy in Documents can be deleted.\n\n"
                + "Free space needed: about " + megabytes(manifest.totalBytes() + SPARE_BYTES)
                + "; free now: " + megabytes(dataDir.getUsableSpace()) + ".");
        LinearLayout row = row(activity, column);
        Button choose = button(activity, row, "Choose GRID folder", v -> pick(activity));
        button(activity, row, "Close", v -> activity.finish());
        show(activity, column, choose);
    }

    private static void resumeScreen(LauncherActivity activity) {
        long remaining = GridImporter.remainingBytes(dataDir, manifest);
        long total = manifest.totalBytes();
        LinearLayout column = column(activity, "The last import did not finish");
        text(activity, column, "Copied " + megabytes(total - remaining) + " of "
                + megabytes(total) + ". Continue from there with the same folder, or choose "
                + "another one.");
        LinearLayout row = row(activity, column);
        Button resume = button(activity, row, "Continue", v -> run(activity, savedTree(activity), false));
        button(activity, row, "Choose GRID folder", v -> pick(activity));
        show(activity, column, resume);
    }

    private static void progressScreen(LauncherActivity activity, String what) {
        LinearLayout column = column(activity, what);
        column.setGravity(Gravity.CENTER);
        ProgressBar progress = new ProgressBar(activity, null, android.R.attr.progressBarStyleHorizontal);
        progress.setMax(1000);
        column.addView(progress, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        TextView line = text(activity, column, "");
        line.setGravity(Gravity.CENTER);
        show(activity, column, null);
        bar = progress;
        status = line;
    }

    private static void problemList(LauncherActivity activity, String titleText, String body,
            List<String> items, boolean offerAnyway) {
        StringBuilder text = new StringBuilder(body);
        int count = Math.min(items.size(), 12);
        for (int i = 0; i < count; i++) {
            text.append("\n  ").append(items.get(i));
        }
        if (items.size() > count) {
            text.append("\n  … and ").append(items.size() - count).append(" more");
        }
        Log.w(TAG, "import: " + titleText + " " + items);
        LinearLayout column = column(activity, titleText);
        text(activity, column, text.toString());
        LinearLayout row = row(activity, column);
        Button choose = button(activity, row, "Choose another folder", v -> pick(activity));
        if (offerAnyway) {
            button(activity, row, "Use these files anyway",
                    v -> run(activity, savedTree(activity), true));
        }
        button(activity, row, "Close", v -> activity.finish());
        show(activity, column, choose);
    }

    /** An error: on screen and in Download/GRID-Reassembly/import-*.txt. */
    private static void problem(LauncherActivity activity, String what, Throwable error,
            boolean offerRetry) {
        StringWriter text = new StringWriter();
        text.append(what).append('\n');
        if (error != null) {
            error.printStackTrace(new PrintWriter(text));
        }
        Log.e(TAG, "import: " + text);
        String logPath = CrashLog.write(activity, "import", text.toString());
        LinearLayout column = column(activity, "The import stopped");
        text(activity, column, what + (error == null ? "" : "\n\n" + error)
                + (logPath == null ? "" : "\n\nLog file: " + logPath));
        LinearLayout row = row(activity, column);
        Button first = null;
        if (offerRetry && savedTree(activity) != null) {
            first = button(activity, row, "Try again", v -> run(activity, savedTree(activity), false));
        }
        Button choose = button(activity, row, "Choose GRID folder", v -> pick(activity));
        button(activity, row, "Close", v -> activity.finish());
        show(activity, column, first != null ? first : choose);
    }

    // ---- Picker ---------------------------------------------------------

    private static void pick(LauncherActivity activity) {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        // Start in Documents, where the steps suggest copying the folder.
        intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, DocumentsContract.buildDocumentUri(
                "com.android.externalstorage.documents", "primary:Documents"));
        try {
            activity.startActivityForResult(intent, PICK);
        } catch (RuntimeException e) {
            problem(activity, "This phone has no folder picker (ACTION_OPEN_DOCUMENT_TREE).", e, false);
        }
    }

    static void picked(LauncherActivity activity, int requestCode, int resultCode, Intent data) {
        if (requestCode != PICK) {
            return;
        }
        Uri tree = data == null ? null : data.getData();
        if (resultCode != Activity.RESULT_OK || tree == null) {
            return; // cancelled: the screen stays
        }
        try {
            activity.getContentResolver().takePersistableUriPermission(
                    tree, Intent.FLAG_GRANT_READ_URI_PERMISSION);
        } catch (SecurityException e) {
            Log.w(TAG, "import: no persistable permission for " + tree, e);
        }
        prefs(activity).edit().putString(TREE, tree.toString()).apply();
        Log.i(TAG, "import: picked " + tree);
        run(activity, tree, false);
    }

    private static SharedPreferences prefs(Activity activity) {
        return activity.getSharedPreferences(PREFS, Activity.MODE_PRIVATE);
    }

    /** The picked folder, while the app may still read it. */
    private static Uri savedTree(Activity activity) {
        String saved = prefs(activity).getString(TREE, null);
        if (saved == null) {
            return null;
        }
        Uri tree = Uri.parse(saved);
        for (UriPermission permission : activity.getContentResolver().getPersistedUriPermissions()) {
            if (permission.getUri().equals(tree) && permission.isReadPermission()) {
                return tree;
            }
        }
        return null;
    }

    // ---- Import ---------------------------------------------------------

    private static void run(LauncherActivity activity, Uri tree, boolean acceptMismatches) {
        if (tree == null) {
            pick(activity);
            return;
        }
        synchronized (ImportFlow.class) {
            if (worker != null) {
                return;
            }
            shown = activity;
            activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            progressScreen(activity, "Checking the GRID folder");
            byte[] settings;
            try {
                settings = readAsset(activity, GridImporter.ASSET_ROOT + "/" + GridImporter.SETTINGS);
            } catch (Exception e) {
                problem(activity, "Cannot read the settings template of this APK.", e, false);
                return;
            }
            SafTree source = new SafTree(activity.getContentResolver(), tree);
            worker = new Thread(() -> work(source, settings, acceptMismatches), "grid-import");
            worker.start();
        }
    }

    private static byte[] readAsset(Activity activity, String path) throws Exception {
        try (InputStream in = activity.getAssets().open(path)) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buffer = new byte[4096];
            int read;
            while ((read = in.read(buffer)) > 0) {
                out.write(buffer, 0, read);
            }
            return out.toByteArray();
        }
    }

    private interface Step {
        void run(LauncherActivity activity);
    }

    /** Runs on the UI thread of the activity currently shown, if any. */
    private static void post(Step step) {
        LauncherActivity activity;
        synchronized (ImportFlow.class) {
            activity = shown;
        }
        if (activity != null) {
            activity.runOnUiThread(() -> {
                if (!activity.isFinishing()) {
                    step.run(activity);
                }
            });
        }
    }

    private static void work(SafTree source, byte[] settings, boolean acceptMismatches) {
        long started = System.nanoTime();
        try {
            GridImporter.Resolver resolver = GridImporter.locate(source);
            if (resolver == null) {
                post(a -> problemList(a, "No GRID folder there",
                        "The picked folder has no grid.exe, and neither has any folder directly "
                                + "inside it. Pick the GRID folder you copied from your installation "
                                + "(the one with grid.exe).", java.util.Collections.emptyList(), false));
                return;
            }
            GridImporter.Check check = GridImporter.check(resolver, manifest);
            if (!check.ok()) {
                java.util.List<String> items = new java.util.ArrayList<>();
                for (String path : check.missing) {
                    items.add("missing: " + path);
                }
                for (String path : check.wrongSize) {
                    items.add("different size: " + path);
                }
                post(a -> problemList(a, "Some game files are missing",
                        "The folder has grid.exe but lacks files the game needs, or has files of "
                                + "another size (another version, or a copy that did not finish). "
                                + "Copy the whole GRID folder again and pick it.", items, false));
                return;
            }
            long remaining = GridImporter.remainingBytes(dataDir, manifest);
            long free = dataDir.getUsableSpace();
            if (free < remaining + SPARE_BYTES) {
                post(a -> problemList(a, "Not enough free space",
                        "The import needs about " + megabytes(remaining + SPARE_BYTES)
                                + " more; " + megabytes(free) + " are free. Free some space and "
                                + "choose the folder again.", java.util.Collections.emptyList(), false));
                return;
            }
            post(a -> progressScreen(a, "Copying the game data"));
            long[] lastShown = {0};
            List<String> mismatches = GridImporter.copy(resolver, manifest, dataDir, acceptMismatches,
                    (done, total, path) -> {
                        long now = System.nanoTime();
                        if (done == total || now - lastShown[0] > 100_000_000L) {
                            lastShown[0] = now;
                            post(a -> showProgress(done, total, path));
                        }
                    });
            if (!mismatches.isEmpty()) {
                post(a -> problemList(a, "Some files differ from the expected version",
                        "These files have the right size but other contents than the Steam version "
                                + "this engine was made for. Copy them again from your installation, "
                                + "or use them anyway (the game may then fail to start):",
                        mismatches, true));
                return;
            }
            post(a -> progressScreen(a, "Checking grid.exe"));
            String error = NativeCheck.deriveTables(new File(dataDir, GridImporter.GRID).getPath());
            if (error != null) {
                post(a -> problem(a, "The game's parameters could not be read from grid.exe: "
                        + error, null, false));
                return;
            }
            GridImporter.finish(dataDir, manifest, settings);
            Log.i(TAG, "import: done in " + (System.nanoTime() - started) / 1_000_000 + " ms"
                    + (acceptMismatches ? " (files with other contents accepted)" : ""));
            post(LauncherActivity::startGame);
        } catch (Throwable e) {
            post(a -> problem(a, "The import failed.", e, true));
        } finally {
            synchronized (ImportFlow.class) {
                worker = null;
            }
        }
    }

    private static void showProgress(long done, long total, String path) {
        if (bar == null) {
            return;
        }
        bar.setProgress(total == 0 ? 1000 : (int) (done * 1000 / total));
        status.setText(String.format(Locale.ROOT, "%d of %d MB\n%s", done >> 20, total >> 20, path));
    }

    // ---- Keys -----------------------------------------------------------

    /**
     * A pad's A presses the focused button (Android also maps it to
     * DPAD_CENTER, but not on every pad); B and Back go back. D-pad and stick
     * focus moves are Android's own.
     */
    static boolean key(LauncherActivity activity, KeyEvent event) {
        if (shown != activity) {
            return false;
        }
        int code = event.getKeyCode();
        if (code == KeyEvent.KEYCODE_BUTTON_A) {
            if (event.getAction() == KeyEvent.ACTION_UP) {
                View focus = activity.getCurrentFocus();
                if (focus != null && focus.isClickable()) {
                    focus.performClick();
                }
            }
            return true;
        }
        if (code == KeyEvent.KEYCODE_BUTTON_B) {
            if (event.getAction() == KeyEvent.ACTION_UP && !back(activity)) {
                activity.finish();
            }
            return true;
        }
        return false;
    }

    /** True when Back is handled here: ignored while the copy runs. */
    static boolean back(LauncherActivity activity) {
        return shown == activity && worker != null;
    }
}
