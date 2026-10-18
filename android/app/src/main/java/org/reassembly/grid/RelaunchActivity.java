package org.reassembly.grid;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Color;
import android.os.Bundle;
import android.os.Process;
import android.os.SystemClock;
import android.util.Log;
import android.view.View;

import java.io.File;

/**
 * Starts the game again after its process ended (event menu Continue and
 * Back to menu, docs/EVENT_MENU.md). Runs in its own process (":relaunch"),
 * waits until the game's process is gone (MainActivity.onDestroy ends it),
 * then opens LauncherActivity, which starts a fresh game process.
 */
public class RelaunchActivity extends Activity {
    static final String EXTRA_PID = "pid";
    /** Longest wait for the old process; it normally ends within a second. */
    private static final long WAIT_MS = 5000;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        View black = new View(this);
        black.setBackgroundColor(Color.BLACK);
        setContentView(black);
        int pid = getIntent().getIntExtra(EXTRA_PID, 0);
        new Thread(() -> {
            long until = SystemClock.uptimeMillis() + WAIT_MS;
            while (pid > 0 && new File("/proc/" + pid).exists()
                    && SystemClock.uptimeMillis() < until) {
                SystemClock.sleep(50);
            }
            if (pid > 0 && new File("/proc/" + pid).exists()) {
                Log.w("grid", "relaunch: old process " + pid + " still running; ending it");
                Process.killProcess(pid);
                SystemClock.sleep(200);
            }
            runOnUiThread(() -> {
                startActivity(new Intent(this, LauncherActivity.class)
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
                // This small process stays cached until Android reclaims it.
                finish();
                overridePendingTransition(0, 0);
            });
        }, "grid-relaunch").start();
    }
}
