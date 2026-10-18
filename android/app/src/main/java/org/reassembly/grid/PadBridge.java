package org.reassembly.grid;

import android.content.Context;
import android.hardware.input.InputManager;
import android.util.Log;
import android.util.SparseArray;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.MotionEvent;

/**
 * Gamepads (P11b, docs/ANDROID.md "Gamepads"): forwards pad connections,
 * buttons and axes to android_pad.rs, which feeds them to Bevy's gamepad
 * input. Everything here runs on the UI thread.
 */
final class PadBridge implements InputManager.InputDeviceListener {
    private static native void nativeConnected(int id, String name, int vendor, int product);
    private static native void nativeDisconnected(int id);
    private static native void nativeKey(int id, int keyCode, boolean down);
    private static native void nativeMotion(
            int id, float lx, float ly, float rx, float ry, float lt, float rt, float hatX, float hatY);

    /** Which Android axes a pad uses for its right stick and triggers. */
    private static final class Layout {
        final int rightX;
        final int rightY;
        final int[] leftTrigger;
        final int[] rightTrigger;
        /** Triggers on -1..1 stick axes (rest -1) instead of 0..1 trigger axes. */
        final boolean stickTriggers;

        Layout(InputDevice device) {
            boolean triggers = has(device, MotionEvent.AXIS_LTRIGGER)
                    || has(device, MotionEvent.AXIS_BRAKE)
                    || has(device, MotionEvent.AXIS_RTRIGGER)
                    || has(device, MotionEvent.AXIS_GAS);
            boolean rxRy = has(device, MotionEvent.AXIS_RX) && has(device, MotionEvent.AXIS_RY);
            // Android's standard pad layout (Xbox, DualSense key layout
            // files): right stick Z/RZ, triggers LTRIGGER/RTRIGGER or
            // BRAKE/GAS. A pad without a key layout file and with RX/RY
            // reports the Linux XInput-style layout: right stick RX/RY,
            // triggers on Z/RZ. AUTHORED heuristic.
            if (!triggers && rxRy) {
                rightX = MotionEvent.AXIS_RX;
                rightY = MotionEvent.AXIS_RY;
                leftTrigger = new int[] {MotionEvent.AXIS_Z};
                rightTrigger = new int[] {MotionEvent.AXIS_RZ};
                stickTriggers = true;
            } else {
                rightX = MotionEvent.AXIS_Z;
                rightY = MotionEvent.AXIS_RZ;
                leftTrigger = new int[] {MotionEvent.AXIS_LTRIGGER, MotionEvent.AXIS_BRAKE};
                rightTrigger = new int[] {MotionEvent.AXIS_RTRIGGER, MotionEvent.AXIS_GAS};
                stickTriggers = false;
            }
        }

        float trigger(MotionEvent event, int[] axes) {
            float v = 0f;
            for (int axis : axes) {
                float a = event.getAxisValue(axis);
                v = Math.max(v, stickTriggers ? (a + 1f) * 0.5f : a);
            }
            return v;
        }

        private static boolean has(InputDevice device, int axis) {
            return device.getMotionRange(axis, InputDevice.SOURCE_JOYSTICK) != null;
        }
    }

    private final Context context;
    private final SparseArray<Layout> pads = new SparseArray<>();
    private InputManager manager;

    PadBridge(Context context) {
        this.context = context;
    }

    static boolean isPad(InputDevice device) {
        if (device == null || device.isVirtual()) {
            return false;
        }
        int sources = device.getSources();
        return (sources & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD
                || (sources & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK;
    }

    void start() {
        manager = context.getSystemService(InputManager.class);
        if (manager == null) {
            return;
        }
        manager.registerInputDeviceListener(this, null);
        for (int id : manager.getInputDeviceIds()) {
            onInputDeviceAdded(id);
        }
    }

    /** In the background no input arrives: release the pads. */
    void stop() {
        if (manager != null) {
            manager.unregisterInputDeviceListener(this);
            manager = null;
        }
        for (int i = 0; i < pads.size(); i++) {
            nativeDisconnected(pads.keyAt(i));
        }
        pads.clear();
    }

    private Layout layout(InputDevice device) {
        Layout layout = pads.get(device.getId());
        if (layout == null) {
            layout = new Layout(device);
            pads.put(device.getId(), layout);
            Log.i("grid", "pad connected id=" + device.getId() + " name=" + device.getName());
            nativeConnected(
                    device.getId(), device.getName(), device.getVendorId(), device.getProductId());
        }
        return layout;
    }

    /**
     * A device announced before it sends input must look like a real pad: an
     * A button or a stick. A built-in device that merely lists a gamepad
     * source would otherwise hide the touch zones (AUTHORED guard); it is
     * still taken once it sends pad input.
     */
    private static boolean announced(InputDevice device) {
        if (!isPad(device)) {
            return false;
        }
        boolean buttonA = device.hasKeys(KeyEvent.KEYCODE_BUTTON_A)[0];
        boolean stick = device.getMotionRange(MotionEvent.AXIS_X, InputDevice.SOURCE_JOYSTICK) != null
                && device.getMotionRange(MotionEvent.AXIS_Y, InputDevice.SOURCE_JOYSTICK) != null;
        return buttonA || stick;
    }

    @Override
    public void onInputDeviceAdded(int id) {
        InputDevice device = InputDevice.getDevice(id);
        if (announced(device)) {
            layout(device);
        }
    }

    @Override
    public void onInputDeviceRemoved(int id) {
        if (pads.get(id) != null) {
            pads.remove(id);
            nativeDisconnected(id);
        }
    }

    @Override
    public void onInputDeviceChanged(int id) {
        onInputDeviceRemoved(id);
        onInputDeviceAdded(id);
    }

    /** True when the event came from a pad and was taken. */
    boolean key(KeyEvent event) {
        InputDevice device = event.getDevice();
        int code = event.getKeyCode();
        boolean padKey = KeyEvent.isGamepadButton(code)
                || code == KeyEvent.KEYCODE_DPAD_UP
                || code == KeyEvent.KEYCODE_DPAD_DOWN
                || code == KeyEvent.KEYCODE_DPAD_LEFT
                || code == KeyEvent.KEYCODE_DPAD_RIGHT
                || code == KeyEvent.KEYCODE_DPAD_CENTER
                || code == KeyEvent.KEYCODE_BACK;
        if (!padKey || !isPad(device)) {
            return false;
        }
        layout(device);
        int action = event.getAction();
        if (action == KeyEvent.ACTION_DOWN && event.getRepeatCount() == 0) {
            nativeKey(device.getId(), code, true);
        } else if (action == KeyEvent.ACTION_UP) {
            nativeKey(device.getId(), code, false);
        }
        return true;
    }

    /** True when the event came from a pad's sticks and was taken. */
    boolean motion(MotionEvent event) {
        if ((event.getSource() & InputDevice.SOURCE_JOYSTICK) != InputDevice.SOURCE_JOYSTICK
                || event.getActionMasked() != MotionEvent.ACTION_MOVE) {
            return false;
        }
        InputDevice device = event.getDevice();
        if (!isPad(device)) {
            return false;
        }
        Layout l = layout(device);
        nativeMotion(
                device.getId(),
                event.getAxisValue(MotionEvent.AXIS_X),
                event.getAxisValue(MotionEvent.AXIS_Y),
                event.getAxisValue(l.rightX),
                event.getAxisValue(l.rightY),
                l.trigger(event, l.leftTrigger),
                l.trigger(event, l.rightTrigger),
                event.getAxisValue(MotionEvent.AXIS_HAT_X),
                event.getAxisValue(MotionEvent.AXIS_HAT_Y));
        return true;
    }
}
