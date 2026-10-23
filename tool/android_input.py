"""Use a real Editable for IME composition, with atomic Unicode state updates."""

def with_name_input(source):
    marker = "    // There is an Android bug when screen is in landscape,"
    if source.count(marker) != 1:
        raise ValueError("miniquad text-input insertion point changed")
    source = source.replace(marker, r'''
    private android.text.Editable xqName = android.text.Editable.Factory.getInstance().newEditable("");
    private void xqNameChanged() {
        int start=Math.max(0,android.text.Selection.getSelectionStart(xqName));
        int end=Math.max(0,android.text.Selection.getSelectionEnd(xqName));
        QuadNative.xiangqiNameEdit(xqName.toString(),Character.codePointCount(xqName,0,start),Character.codePointCount(xqName,0,end));
    }
    public void xiangqiNameState(String text,int start,int end,boolean visible) {
        if(!visible)android.view.inputmethod.BaseInputConnection.removeComposingSpans(xqName);
        else if(android.view.inputmethod.BaseInputConnection.getComposingSpanStart(xqName)>=0)return;
        if(!xqName.toString().equals(text))xqName.replace(0,xqName.length(),text);
        start=Math.max(0,Math.min(start,text.codePointCount(0,text.length())));
        end=Math.max(0,Math.min(end,text.codePointCount(0,text.length())));
        android.text.Selection.setSelection(xqName,text.offsetByCodePoints(0,start),text.offsetByCodePoints(0,end));
    }
    @Override public boolean onCheckIsTextEditor() { return true; }
''' + marker)
    original = """        InputConnection connection = super.onCreateInputConnection(outAttrs);
        outAttrs.imeOptions |= EditorInfo.IME_FLAG_NO_FULLSCREEN;
        return connection;"""
    replacement = r'''        outAttrs.inputType = android.text.InputType.TYPE_CLASS_TEXT;
        outAttrs.imeOptions = EditorInfo.IME_ACTION_DONE | EditorInfo.IME_FLAG_NO_FULLSCREEN;
        outAttrs.initialSelStart = Math.max(0,android.text.Selection.getSelectionStart(xqName));
        outAttrs.initialSelEnd = Math.max(0,android.text.Selection.getSelectionEnd(xqName));
        xqName.setFilters(new android.text.InputFilter[]{(text,start,end,dest,dstart,dend) -> {
            int available=40-Character.codePointCount(dest,0,dest.length())+Character.codePointCount(dest,dstart,dend);
            int requested=Character.codePointCount(text,start,end);
            if(requested<=available)return null;
            return text.subSequence(start,Character.offsetByCodePoints(text,start,Math.max(0,available)));
        }});
        return new android.view.inputmethod.BaseInputConnection(this,true) {
            @Override public android.text.Editable getEditable() { return xqName; }
            @Override public boolean commitText(CharSequence text,int position) { boolean ok=super.commitText(text,position);xqNameChanged();return ok; }
            @Override public boolean setComposingText(CharSequence text,int position) { boolean ok=super.setComposingText(text,position);xqNameChanged();return ok; }
            @Override public boolean finishComposingText() { boolean ok=super.finishComposingText();xqNameChanged();return ok; }
            @Override public boolean setSelection(int start,int end) { boolean ok=super.setSelection(start,end);xqNameChanged();return ok; }
            @Override public boolean deleteSurroundingText(int before,int after) {
                int a=Math.max(0,Math.min(android.text.Selection.getSelectionStart(xqName),android.text.Selection.getSelectionEnd(xqName)));
                int b=Math.max(a,Math.max(android.text.Selection.getSelectionStart(xqName),android.text.Selection.getSelectionEnd(xqName)));
                int left=Math.max(0,a-before),right=Math.min(xqName.length(),b+after);
                if(left>0 && left<xqName.length() && Character.isLowSurrogate(xqName.charAt(left)) && Character.isHighSurrogate(xqName.charAt(left-1)))before++;
                if(right>0 && right<xqName.length() && Character.isHighSurrogate(xqName.charAt(right-1)) && Character.isLowSurrogate(xqName.charAt(right)))after++;
                boolean ok=super.deleteSurroundingText(before,after);xqNameChanged();return ok;
            }
            @Override public boolean deleteSurroundingTextInCodePoints(int before,int after) { boolean ok=super.deleteSurroundingTextInCodePoints(before,after);xqNameChanged();return ok; }
            @Override public boolean performEditorAction(int action) {
                if(action==EditorInfo.IME_ACTION_DONE){finishComposingText();QuadNative.surfaceOnKeyDown(KeyEvent.KEYCODE_ENTER);QuadNative.surfaceOnKeyUp(KeyEvent.KEYCODE_ENTER);return true;}
                return super.performEditorAction(action);
            }
        };'''
    if source.count(original) != 1:
        raise ValueError("miniquad input connection template changed")
    source = source.replace(original, replacement)
    marker = "    //% MAIN_ACTIVITY_BODY"
    if source.count(marker) != 1:
        raise ValueError("name editor activity insertion point changed")
    return source.replace(marker, marker + '''
    public void xiangqiNameState(String text,int start,int end,boolean visible) { runOnUiThread(() -> view.xiangqiNameState(text,start,end,visible)); }
''')
