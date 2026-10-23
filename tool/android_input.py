"""Install a committing IME connection in the pinned miniquad Java bridge."""

def with_name_input(source):
    marker = "    // There is an Android bug when screen is in landscape,"
    if source.count(marker) != 1:
        raise ValueError("miniquad text-input insertion point changed")
    source = source.replace(marker, """    @Override
    public boolean onCheckIsTextEditor() { return true; }

""" + marker)
    original = """        InputConnection connection = super.onCreateInputConnection(outAttrs);
        outAttrs.imeOptions |= EditorInfo.IME_FLAG_NO_FULLSCREEN;
        return connection;"""
    replacement = """        outAttrs.inputType = android.text.InputType.TYPE_CLASS_TEXT;
        outAttrs.imeOptions = EditorInfo.IME_ACTION_DONE | EditorInfo.IME_FLAG_NO_FULLSCREEN;
        return new android.view.inputmethod.BaseInputConnection(this, false) {
            private String composing = "";
            private void replaceComposing(CharSequence text) {
                int count = composing.codePointCount(0, composing.length());
                for (int i = 0; i < count; i++) QuadNative.surfaceOnCharacter(8);
                text.codePoints().forEach(cp -> QuadNative.surfaceOnCharacter(cp));
            }
            @Override
            public boolean commitText(CharSequence text, int newCursorPosition) {
                replaceComposing(text);
                composing = "";
                return true;
            }
            @Override
            public boolean setComposingText(CharSequence text, int newCursorPosition) {
                replaceComposing(text);
                composing = text.toString();
                return true;
            }
            @Override
            public boolean finishComposingText() {
                composing = "";
                return true;
            }
            @Override
            public boolean deleteSurroundingText(int beforeLength, int afterLength) {
                for (int i = 0; i < Math.min(beforeLength, 40); i++) QuadNative.surfaceOnCharacter(8);
                for (int i = 0; i < Math.min(afterLength, 40); i++) QuadNative.surfaceOnCharacter(127);
                return true;
            }
            @Override
            public boolean performEditorAction(int actionCode) {
                if (actionCode == EditorInfo.IME_ACTION_DONE) {
                    QuadNative.surfaceOnKeyDown(KeyEvent.KEYCODE_ENTER);
                    QuadNative.surfaceOnKeyUp(KeyEvent.KEYCODE_ENTER);
                    return true;
                }
                return super.performEditorAction(actionCode);
            }
        };"""
    if source.count(original) != 1:
        raise ValueError("miniquad input connection template changed")
    return source.replace(original, replacement)
