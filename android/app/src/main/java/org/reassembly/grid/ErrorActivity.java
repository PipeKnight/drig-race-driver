package org.reassembly.grid;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Color;
import android.os.Bundle;
import android.os.Process;
import android.view.Gravity;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

/**
 * Shows why the game stopped (docs/ANDROID.md). The native code calls
 * {@link MainActivity#showFatal} on a start-up error or a panic. This activity
 * runs in its own process (":error" in the manifest), so it stays on screen
 * when the game process exits or aborts right after.
 */
public class ErrorActivity extends Activity {
    static final String EXTRA_MESSAGE = "message";
    static final String EXTRA_LOG = "log";
    static final String EXTRA_PID = "pid";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Intent intent = getIntent();
        String message = intent.getStringExtra(EXTRA_MESSAGE);
        String log = intent.getStringExtra(EXTRA_LOG);
        int pid = intent.getIntExtra(EXTRA_PID, 0);

        float density = getResources().getDisplayMetrics().density;
        int pad = (int) (24 * density);
        LinearLayout column = new LinearLayout(this);
        column.setOrientation(LinearLayout.VERTICAL);
        column.setBackgroundColor(Color.BLACK);
        column.setPadding(pad, pad, pad, pad);

        TextView title = new TextView(this);
        title.setText("GRID Reassembly stopped");
        title.setTextColor(Color.rgb(255, 120, 90));
        title.setTextSize(22);
        column.addView(title);

        TextView body = new TextView(this);
        body.setTextColor(Color.WHITE);
        body.setTextIsSelectable(true);
        body.setPadding(0, pad / 2, 0, pad / 2);
        body.setText((message == null ? "(no message)" : message) + "\n\n"
                + (log == null ? "No log file could be written (see logcat, tag grid)."
                        : "Log file: " + log + " (open it with Files / File Manager)")
                + "\n\nIf the message names a missing game file, this APK carries no game"
                + " data: install the bundled APK or push the data (docs/ANDROID.md).");
        ScrollView scroll = new ScrollView(this);
        scroll.addView(body);
        column.addView(scroll, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));

        Button close = new Button(this);
        close.setText("Close");
        close.setGravity(Gravity.CENTER);
        close.setOnClickListener(view -> {
            // The game process cannot start the native library twice; make
            // sure it is gone so the next start is clean (same app uid).
            if (pid > 0 && pid != Process.myPid()) {
                Process.killProcess(pid);
            }
            finishAndRemoveTask();
            Process.killProcess(Process.myPid());
        });
        column.addView(close);
        setContentView(column);
    }
}
