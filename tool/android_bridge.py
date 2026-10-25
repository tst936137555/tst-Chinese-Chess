"""Generate the exact Java sources used by APK builds and compile regressions."""
from android_input import with_name_input
from android_accessibility import with_accessibility


def activity_source(template, package="com.tst.xiangqi.rust"):
    if template.count("QuadNative.activityOnCreate(this);") != 1:
        raise ValueError("miniquad activity initialization template changed")
    main_activity = template.replace("TARGET_PACKAGE_NAME", package).replace("LIBRARY_NAME", "tst_xiangqi")
    main_activity = main_activity.replace("//% MAIN_ACTIVITY_ON_CREATE", "// Keep-awake follows the game state, rather than every foreground page.")
    main_activity = main_activity.replace("    //% MAIN_ACTIVITY_BODY", "    //% MAIN_ACTIVITY_BODY\n    public void xiangqiKeepAwake(boolean enabled) { runOnUiThread(() -> { if(enabled) getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON); else getWindow().clearFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON); }); }")
    # Automation writes only isolated external app files, so the exact non-debuggable
    # release APK can be exercised without run-as or access to normal user saves.
    main_activity = main_activity.replace("QuadNative.activityOnCreate(this);", 'QuadNative.xiangqiInit();\n        java.io.File automationRoot=getExternalFilesDir(null);\n        if (automationRoot!=null && getIntent().getBooleanExtra("xiangqiSmoke", false)) { QuadNative.xiangqiSmoke(automationRoot.getAbsolutePath() + "/smoke"); }\n        if (automationRoot!=null && getIntent().getBooleanExtra("xiangqiObserve", false)) { QuadNative.xiangqiObserve(automationRoot.getAbsolutePath() + "/interaction/interaction.json"); }\n        view.addOnLayoutChangeListener((v,l,t,r,b,ol,ot,orr,ob) -> { int[] xy = new int[2]; view.getLocationOnScreen(xy); QuadNative.xiangqiViewOrigin(xy[0], xy[1]); });\n        QuadNative.activityOnCreate(this);')
    main_activity = main_activity.replace("//% MAIN_ACTIVITY_ON_PAUSE", "QuadNative.xiangqiPaused(true);")
    main_activity = main_activity.replace("//% MAIN_ACTIVITY_ON_RESUME", "QuadNative.xiangqiPaused(false);")
    main_activity = main_activity.replace("        super.onBackPressed();", "        QuadNative.xiangqiBack();")
    main_activity = main_activity.replace("        int pointerCount = event.getPointerCount();", '        if (((Activity)getContext()).getIntent().getBooleanExtra("xiangqiObserve", false)) Log.i("XQ_INPUT", "surface action=" + event.getActionMasked() + " x=" + event.getX() + " y=" + event.getY());\n        int pointerCount = event.getPointerCount();')
    main_activity = main_activity.replace("    //% MAIN_ACTIVITY_BODY", '    //% MAIN_ACTIVITY_BODY\n    @Override public boolean dispatchTouchEvent(MotionEvent event) {\n        if (getIntent().getBooleanExtra("xiangqiObserve", false)) Log.i("XQ_INPUT", "activity action=" + event.getActionMasked() + " x=" + event.getX() + " y=" + event.getY());\n        return super.dispatchTouchEvent(event);\n    }')
    # Leave Back dispatch to Activity/OnBackInvokedDispatcher. Forwarding it as
    # a game key as well can close a newly opened dialog with a second event.
    main_activity = main_activity.replace("    public boolean onKey(View v, int keyCode, KeyEvent event) {", "    public boolean onKey(View v, int keyCode, KeyEvent event) {\n        if (keyCode == KeyEvent.KEYCODE_BACK) return false;")
    main_activity = main_activity.replace("    //% MAIN_ACTIVITY_BODY", "    //% MAIN_ACTIVITY_BODY\n    public void xiangqiBackground() { runOnUiThread(() -> moveTaskToBack(true)); }\n    @Override protected void onNewIntent(Intent intent) {\n        super.onNewIntent(intent);\n        if (getIntent().getBooleanExtra(\"xiangqiObserve\", false) && intent.getBooleanExtra(\"xiangqiRecreate\", false)) recreate();\n    }")
    main_activity = main_activity.replace("QuadNative.xiangqiInit();", "QuadNative.xiangqiInit();\n        if (android.os.Build.VERSION.SDK_INT >= 33) { getOnBackInvokedDispatcher().registerOnBackInvokedCallback(android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT, () -> QuadNative.xiangqiBack()); }")
    # Include display cutouts, including landscape notches, in the template's safe padding.
    main_activity = main_activity.replace("insets.getInsets(WindowInsets.Type.systemBars())", "insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout())")
    return with_accessibility(with_name_input(main_activity))


def native_source(template):
    if template.count("public class QuadNative {") != 1:
        raise ValueError("miniquad native bridge template changed")
    quad_native = template
    quad_native = quad_native.replace("public class QuadNative {", "public class QuadNative {\n    public native static void xiangqiAccessibilityActivate(int id);\n    public native static void xiangqiNameEdit(String text,int start,int end);")
    quad_native = quad_native.replace("public class QuadNative {", "public class QuadNative {\n    public native static void xiangqiInit();\n    public native static void xiangqiObserve(String path);\n    public native static void xiangqiViewOrigin(int x,int y);\n    public native static void xiangqiPaused(boolean paused);\n    public native static void xiangqiBack();\n    public native static void xiangqiSmoke(String path);")
    return quad_native
