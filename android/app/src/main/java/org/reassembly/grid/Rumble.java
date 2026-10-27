package org.reassembly.grid;

import android.content.Context;
import android.os.CombinedVibration;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.os.VibratorManager;
import android.view.InputDevice;

/**
 * Pad rumble and phone vibration (AUTHORED, docs/FORCE_FEEDBACK.md section 7.2): the
 * original's two XInput motor levels from force_feedback/android.rs, as 100 ms one-shots
 * that the Rust side renews while they last. A connected pad with vibrators gets them,
 * left motor on its first vibrator and right on its second; a pad without vibrators gets
 * nothing; with no pad the phone vibrates at the larger level. Called on the UI thread.
 */
final class Rumble {
    private static final long LENGTH_MS = 100;

    private final Context context;
    /** The pad that vibrated last, or -1 for the phone, or -2 for none. */
    private int playing = -2;

    Rumble(Context context) {
        this.context = context;
    }

    void set(int left, int right, int pad) {
        if (pad != playing) {
            stop();
        }
        if (left <= 0 && right <= 0) {
            stop();
            return;
        }
        if (pad >= 0) {
            VibratorManager manager = padVibrators(pad);
            if (manager == null) {
                return;
            }
            int[] ids = manager.getVibratorIds();
            CombinedVibration.ParallelCombination both = CombinedVibration.startParallel();
            if (ids.length == 1) {
                both.addVibrator(ids[0], effect(Math.max(left, right), true));
            } else {
                if (left > 0) {
                    both.addVibrator(ids[0], effect(left, true));
                }
                if (right > 0) {
                    both.addVibrator(ids[1], effect(right, true));
                }
            }
            manager.vibrate(both.combine());
        } else {
            Vibrator phone = phone();
            if (phone == null) {
                return;
            }
            int level = Math.max(left, right);
            // Without amplitude control a phone motor is on or off: on from half [A].
            if (!phone.hasAmplitudeControl() && level < 128) {
                phone.cancel();
                playing = -1;
                return;
            }
            phone.vibrate(effect(level, phone.hasAmplitudeControl()));
        }
        playing = pad >= 0 ? pad : -1;
    }

    private void stop() {
        if (playing >= 0) {
            VibratorManager manager = padVibrators(playing);
            if (manager != null) {
                manager.cancel();
            }
        } else if (playing == -1) {
            Vibrator phone = phone();
            if (phone != null) {
                phone.cancel();
            }
        }
        playing = -2;
    }

    private static VibrationEffect effect(int level, boolean amplitude) {
        int a = amplitude ? Math.min(255, Math.max(1, level)) : VibrationEffect.DEFAULT_AMPLITUDE;
        return VibrationEffect.createOneShot(LENGTH_MS, a);
    }

    private static VibratorManager padVibrators(int pad) {
        InputDevice device = InputDevice.getDevice(pad);
        if (device == null) {
            return null;
        }
        VibratorManager manager = device.getVibratorManager();
        return manager.getVibratorIds().length > 0 ? manager : null;
    }

    private Vibrator phone() {
        VibratorManager manager = context.getSystemService(VibratorManager.class);
        if (manager == null) {
            return null;
        }
        Vibrator vibrator = manager.getDefaultVibrator();
        return vibrator.hasVibrator() ? vibrator : null;
    }
}
