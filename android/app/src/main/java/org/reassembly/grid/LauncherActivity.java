package org.reassembly.grid;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Color;
import android.os.Bundle;
import android.util.Log;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.WindowManager;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.ScrollView;
import android.widget.TextView;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.PrintWriter;
import java.io.Reader;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;

/**
 * Start screen (docs/ANDROID.md). A private APK built with
 * {@code tools/build_android.ps1 -BundleData} carries the owner's game data as
 * assets; this activity copies it to the data folder on the first start (and
 * after the bundled data changes), with a progress bar, before the game
 * (MainActivity) loads the native library. With an up-to-date copy it starts
 * the game at once. Without bundled data (the code-only APK) it hands over to
 * {@link ImportFlow}, which imports the user's own GRID files (P10).
 */
public class LauncherActivity extends Activity {
    private static final String TAG = "grid";
    /** One copy per process, even if the activity is recreated meanwhile. */
    private static Thread worker;
    private static LauncherActivity shown;

    private ProgressBar bar;
    private TextView status;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        File dataDir = getExternalFilesDir(null);
        BundleExtractor.Manifest manifest;
        try {
            manifest = readManifest();
        } catch (Exception e) {
            fail("Cannot read the bundled game data list", e);
            return;
        }
        if (manifest == null) {
            // The code-only APK: import the user's own files (P10).
            ImportFlow.start(this, dataDir);
            return;
        }
        if (worker == null && dataDir != null && BundleExtractor.upToDate(dataDir, manifest)) {
            startGame();
            return;
        }
        if (dataDir == null) {
            fail("The app's storage folder is not available (shared storage not mounted?)", null);
            return;
        }
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        buildProgressView();
        synchronized (LauncherActivity.class) {
            shown = this;
            if (worker == null) {
                worker = new Thread(() -> copy(manifest, dataDir), "grid-extract");
                worker.start();
            }
        }
    }

    @Override
    protected void onDestroy() {
        synchronized (LauncherActivity.class) {
            if (shown == this) {
                shown = null;
            }
        }
        ImportFlow.detach(this);
        super.onDestroy();
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        ImportFlow.picked(this, requestCode, resultCode, data);
    }

    /** Pads on the import screens: A presses the focused button, B is Back. */
    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        if (ImportFlow.key(this, event)) {
            return true;
        }
        return super.dispatchKeyEvent(event);
    }

    @Override
    public void onBackPressed() {
        if (!ImportFlow.back(this)) {
            super.onBackPressed();
        }
    }

    /** null when the APK has no bundled data (the default build). */
    private BundleExtractor.Manifest readManifest() throws Exception {
        InputStream in;
        try {
            in = getAssets().open(BundleExtractor.ASSET_ROOT + "/" + BundleExtractor.MANIFEST);
        } catch (FileNotFoundException e) {
            return null;
        }
        try (Reader reader = new InputStreamReader(in, StandardCharsets.UTF_8)) {
            return BundleExtractor.parse(reader);
        }
    }

    private void copy(BundleExtractor.Manifest manifest, File dataDir) {
        long started = System.nanoTime();
        long[] lastShown = {0};
        try {
            Log.i(TAG, "extract: " + manifest.entries.size() + " files, " + manifest.totalBytes()
                    + " bytes, version " + manifest.version + " -> " + dataDir);
            BundleExtractor.extract(
                    path -> getAssets().open(BundleExtractor.ASSET_ROOT + "/" + path),
                    manifest, dataDir, (done, total) -> {
                        long now = System.nanoTime();
                        if (done == total || now - lastShown[0] > 100_000_000L) {
                            lastShown[0] = now;
                            post(activity -> activity.showProgress(done, total));
                        }
                    });
            Log.i(TAG, "extract: done in " + (System.nanoTime() - started) / 1_000_000 + " ms");
            post(LauncherActivity::startGame);
        } catch (Exception e) {
            Log.e(TAG, "extract failed", e);
            post(activity -> activity.fail("Copying the bundled game data failed", e));
        } finally {
            synchronized (LauncherActivity.class) {
                worker = null;
            }
        }
    }

    private interface Step {
        void run(LauncherActivity activity);
    }

    /** Runs on the UI thread of the activity currently shown, if any. */
    private static void post(Step step) {
        LauncherActivity activity;
        synchronized (LauncherActivity.class) {
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

    private void buildProgressView() {
        LinearLayout column = new LinearLayout(this);
        column.setOrientation(LinearLayout.VERTICAL);
        column.setGravity(Gravity.CENTER);
        column.setBackgroundColor(Color.BLACK);
        int pad = (int) (32 * getResources().getDisplayMetrics().density);
        column.setPadding(pad, pad, pad, pad);
        TextView title = new TextView(this);
        title.setText("GRID Reassembly\nFirst start: copying the game data");
        title.setTextColor(Color.WHITE);
        title.setTextSize(20);
        title.setGravity(Gravity.CENTER);
        column.addView(title);
        bar = new ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal);
        bar.setMax(1000);
        column.addView(bar, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        status = new TextView(this);
        status.setTextColor(Color.LTGRAY);
        status.setGravity(Gravity.CENTER);
        column.addView(status);
        setContentView(column);
    }

    private void showProgress(long done, long total) {
        if (bar == null) {
            return;
        }
        bar.setProgress(total == 0 ? 1000 : (int) (done * 1000 / total));
        status.setText(String.format(java.util.Locale.ROOT, "%d of %d MB",
                done >> 20, total >> 20));
    }

    void startGame() {
        startActivity(new Intent(this, MainActivity.class));
        finish();
    }

    /** Shows the error on screen and writes it to Download/GRID-Reassembly. */
    private void fail(String what, Throwable error) {
        StringWriter text = new StringWriter();
        text.append(what).append('\n');
        if (error != null) {
            error.printStackTrace(new PrintWriter(text));
        }
        Log.e(TAG, text.toString());
        String logPath = CrashLog.write(this, "extract", text.toString());
        TextView view = new TextView(this);
        view.setTextColor(Color.WHITE);
        view.setBackgroundColor(Color.BLACK);
        view.setTextIsSelectable(true);
        int pad = (int) (24 * getResources().getDisplayMetrics().density);
        view.setPadding(pad, pad, pad, pad);
        view.setText(what + "\n\n" + (error == null ? "" : error + "\n\n")
                + (logPath == null ? "No log file could be written."
                        : "Log file: " + logPath)
                + "\n\nFree some storage and start the app again; the copy restarts.");
        ScrollView scroll = new ScrollView(this);
        scroll.setBackgroundColor(Color.BLACK);
        scroll.addView(view);
        setContentView(scroll);
    }
}
