package org.reassembly.grid;

import android.content.ContentResolver;
import android.content.ContentUris;
import android.content.ContentValues;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.os.Environment;
import android.os.ParcelFileDescriptor;
import android.provider.MediaStore;
import android.util.Log;

import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.Locale;
import java.util.TimeZone;

/**
 * Log files in the shared Downloads folder, Download/GRID-Reassembly/, where
 * the owner can open them with any file manager (docs/ANDROID.md). Written
 * through MediaStore, which needs no permission on Android 10 and newer. The
 * oldest files beyond {@link #KEEP} are deleted. Names follow the desktop run
 * logs (player.rs setup_output): {@code <prefix>-<UTC yyyyMMdd'T'HHmmss'Z'>-<revision>.txt},
 * the revision being the build's {@code GRID_BUILD_REVISION} (12 hex digits).
 */
final class CrashLog {
    static final String FOLDER = Environment.DIRECTORY_DOWNLOADS + "/GRID-Reassembly/";
    static final int KEEP = 10;
    private static final String TAG = "grid";

    /** A new log file: its descriptor and the path the owner sees. */
    static final class LogFile {
        final ParcelFileDescriptor descriptor;
        final String shownPath;

        LogFile(ParcelFileDescriptor descriptor, String shownPath) {
            this.descriptor = descriptor;
            this.shownPath = shownPath;
        }
    }

    private CrashLog() {}

    /**
     * {@code <prefix>-<UTC time>-<revision>.txt}: letters, digits and '-' of
     * the revision only, "unversioned" when none is left.
     */
    static String fileName(String prefix, long epochMillis, String revision) {
        SimpleDateFormat format = new SimpleDateFormat("yyyyMMdd'T'HHmmss'Z'", Locale.ROOT);
        format.setTimeZone(TimeZone.getTimeZone("UTC"));
        String tag = revision == null ? "" : revision.replaceAll("[^0-9A-Za-z-]", "");
        return prefix + "-" + format.format(new Date(epochMillis)) + "-"
                + (tag.isEmpty() ? "unversioned" : tag) + ".txt";
    }

    /** Creates {@code <prefix>-<time>-<revision>.txt}; null if MediaStore refuses. */
    static LogFile create(Context context, String prefix) {
        try {
            prune(context);
            String name = fileName(prefix, System.currentTimeMillis(), BuildConfig.GRID_REVISION);
            ContentValues values = new ContentValues();
            values.put(MediaStore.MediaColumns.DISPLAY_NAME, name);
            values.put(MediaStore.MediaColumns.MIME_TYPE, "text/plain");
            values.put(MediaStore.MediaColumns.RELATIVE_PATH, FOLDER);
            ContentResolver resolver = context.getContentResolver();
            Uri uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values);
            if (uri == null) {
                Log.e(TAG, "log file: MediaStore insert returned null");
                return null;
            }
            ParcelFileDescriptor descriptor = resolver.openFileDescriptor(uri, "w");
            if (descriptor == null) {
                return null;
            }
            // MediaStore may rename on a clash ("name (1).txt").
            String shown = name;
            try (Cursor cursor = resolver.query(uri,
                    new String[] {MediaStore.MediaColumns.DISPLAY_NAME}, null, null, null)) {
                if (cursor != null && cursor.moveToFirst()) {
                    shown = cursor.getString(0);
                }
            }
            return new LogFile(descriptor, FOLDER + shown);
        } catch (Exception e) {
            Log.e(TAG, "log file: " + e);
            return null;
        }
    }

    /** Writes {@code text} to a new log file; returns the shown path or null. */
    static String write(Context context, String prefix, String text) {
        LogFile file = create(context, prefix);
        if (file == null) {
            return null;
        }
        try (FileOutputStream out = new FileOutputStream(file.descriptor.getFileDescriptor())) {
            out.write(text.getBytes(StandardCharsets.UTF_8));
        } catch (IOException e) {
            Log.e(TAG, "log file: " + e);
        } finally {
            try {
                file.descriptor.close();
            } catch (IOException ignored) {
                // Nothing else to do; the text is in logcat too.
            }
        }
        return file.shownPath;
    }

    /** Keeps the newest KEEP - 1 files, so the new one makes KEEP. */
    private static void prune(Context context) {
        ContentResolver resolver = context.getContentResolver();
        List<Uri> old = new ArrayList<>();
        // Without a storage permission the query returns only this app's files.
        try (Cursor cursor = resolver.query(MediaStore.Downloads.EXTERNAL_CONTENT_URI,
                new String[] {MediaStore.MediaColumns._ID},
                MediaStore.MediaColumns.RELATIVE_PATH + "=?", new String[] {FOLDER},
                MediaStore.MediaColumns.DATE_ADDED + " DESC")) {
            if (cursor == null) {
                return;
            }
            int index = 0;
            while (cursor.moveToNext()) {
                if (++index >= KEEP) {
                    old.add(ContentUris.withAppendedId(MediaStore.Downloads.EXTERNAL_CONTENT_URI,
                            cursor.getLong(0)));
                }
            }
        }
        for (Uri uri : old) {
            try {
                resolver.delete(uri, null, null);
            } catch (Exception e) {
                Log.w(TAG, "log file prune: " + e);
            }
        }
    }
}
