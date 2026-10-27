package org.reassembly.grid;

import android.content.Intent;
import android.os.Bundle;
import android.os.Process;
import android.util.Log;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;

import com.google.androidgamesdk.GameActivity;

/** Starts the Rust prototype (libgrid_android.so, docs/ANDROID.md). */
public class MainActivity extends GameActivity {
    static {
        System.loadLibrary("grid_android");
    }

    private volatile String logPath;
    private final PadBridge pads = new PadBridge(this);
    private final Rumble rumble = new Rumble(this);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        // A lap has no touch input yet (P11); keep the screen on while driving.
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
    }

    @Override
    protected void onStart() {
        super.onStart();
        pads.start();
    }

    @Override
    protected void onStop() {
        pads.stop();
        super.onStop();
    }

    // Gamepads (P11b): winit would turn stick motion into fake touches and
    // drop the other axes, so pad events go to android_pad.rs instead and
    // never reach GameActivity, winit or Android's DPAD/BACK fallbacks.
    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        return pads.key(event) || super.dispatchKeyEvent(event);
    }

    @Override
    public boolean dispatchGenericMotionEvent(MotionEvent event) {
        return pads.motion(event) || super.dispatchGenericMotionEvent(event);
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            WindowInsetsController controller = getWindow().getInsetsController();
            if (controller != null) {
                controller.hide(WindowInsets.Type.systemBars());
                controller.setSystemBarsBehavior(
                        WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        }
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        // The native library (winit, Bevy) cannot start twice in one process;
        // end the process so the next start is a fresh one.
        if (isFinishing()) {
            Process.killProcess(Process.myPid());
        }
    }

    /**
     * Called by android.rs (event menu Continue, Back to menu): ends this
     * process and starts the game again in a new one, through
     * RelaunchActivity in its own process (the native library cannot start
     * twice in one process).
     */
    public void relaunch() {
        runOnUiThread(() -> {
            startActivity(new Intent(this, RelaunchActivity.class)
                    .putExtra(RelaunchActivity.EXTRA_PID, Process.myPid())
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            finish();
        });
    }

    /**
     * Called by android.rs first at start-up (F451). With bundled game data,
     * true only when its copy is complete (the version marker matches). Else
     * the copy screen (LauncherActivity) opens in place of this activity and
     * false is returned: Android can bring MainActivity back directly, from
     * Recents or the task after an update, without the copy screen, and the
     * owner's phone then ran menus and races on half-copied data (19.10).
     */
    public boolean dataReady() {
        try {
            BundleExtractor.Manifest manifest = BundleExtractor.bundled(this);
            java.io.File dataDir = getExternalFilesDir(null);
            if (manifest == null || (dataDir != null && BundleExtractor.upToDate(dataDir, manifest))) {
                return true;
            }
        } catch (Exception e) {
            Log.e("grid", "dataReady: cannot read the bundled data list", e);
            return true;
        }
        Log.w("grid", "dataReady: bundled data not (fully) copied; opening the copy screen");
        runOnUiThread(() -> {
            startActivity(new Intent(this, LauncherActivity.class)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
            finish();
        });
        return false;
    }

    /**
     * Called by android.rs at start-up: a new log file in
     * Download/GRID-Reassembly; returns its file descriptor (now owned by the
     * caller) or -1.
     */
    public int openLogFile() {
        CrashLog.LogFile file = CrashLog.create(this, "log");
        if (file == null) {
            return -1;
        }
        logPath = file.shownPath;
        return file.descriptor.detachFd();
    }

    /**
     * Called by the FFB thread (force_feedback/android.rs): vibration levels 0..255 for a pad's
     * left and right motor, or the phone's vibrator when no pad is connected (Rumble.java).
     */
    public void rumble(int left, int right) {
        runOnUiThread(() -> rumble.set(left, right, pads.first()));
    }

    /**
     * Called by android.rs, from any thread, when the game stops on a
     * start-up error or a panic: shows the message in ErrorActivity, which
     * runs in its own process and outlives this one.
     */
    public void showFatal(String message) {
        Log.e("grid", "fatal: " + message);
        Intent intent = new Intent(this, ErrorActivity.class)
                .putExtra(ErrorActivity.EXTRA_MESSAGE, message)
                .putExtra(ErrorActivity.EXTRA_LOG, logPath)
                .putExtra(ErrorActivity.EXTRA_PID, Process.myPid())
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        startActivity(intent);
    }
}
