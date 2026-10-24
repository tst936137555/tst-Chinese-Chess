"""Expose the shared semantic tree as Android virtual accessibility descendants."""
def with_accessibility(source):
    marker = "    // There is an Android bug when screen is in landscape,"
    if source.count(marker) != 1:
        raise ValueError("accessibility insertion point changed")
    body = r'''
    private org.json.JSONObject xqTree = new org.json.JSONObject();
    private int xqFocused = -1, xqHovered = -1;
    private org.json.JSONObject xqElement(int id) {
        org.json.JSONArray nodes = xqTree.optJSONArray("elements");
        if (nodes != null) for (int i=0;i<nodes.length();i++) {
            org.json.JSONObject node=nodes.optJSONObject(i);
            if (node != null && node.optInt("id")==id) return node;
        }
        return null;
    }
    private android.graphics.Rect xqBounds(org.json.JSONObject element) {
        org.json.JSONArray r=element.optJSONArray("rect");
        float scale=(float)xqTree.optDouble("dpi",1.0);
        return new android.graphics.Rect(Math.round((float)r.optDouble(0)*scale),Math.round((float)r.optDouble(1)*scale),
            Math.round((float)(r.optDouble(0)+r.optDouble(2))*scale),Math.round((float)(r.optDouble(1)+r.optDouble(3))*scale));
    }
    private void xqEvent(int id,int type) {
        android.view.accessibility.AccessibilityEvent event=android.view.accessibility.AccessibilityEvent.obtain(type);
        event.setSource(this,id); event.setPackageName(getContext().getPackageName());
        org.json.JSONObject node=xqElement(id);
        if (node!=null) event.setContentDescription(node.optString("label"));
        if(getParent()!=null)getParent().requestSendAccessibilityEvent(this,event);
    }
    public void xiangqiUpdateAccessibility(String json) {
        try { xqTree=new org.json.JSONObject(json); } catch(org.json.JSONException e) { return; }
        setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
        setContentDescription(xqTree.optString("summary"));
        if(xqFocused!=-1 && xqElement(xqFocused)==null) xqFocused=-1;
        sendAccessibilityEvent(android.view.accessibility.AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
    }
    @Override public boolean dispatchHoverEvent(MotionEvent event) {
        android.view.accessibility.AccessibilityManager manager=(android.view.accessibility.AccessibilityManager)getContext().getSystemService(Context.ACCESSIBILITY_SERVICE);
        if(manager==null || !manager.isTouchExplorationEnabled()) return super.dispatchHoverEvent(event);
        int hovered=-1;
        org.json.JSONArray nodes=xqTree.optJSONArray("elements");
        if(event.getAction()!=MotionEvent.ACTION_HOVER_EXIT && nodes!=null) for(int i=0;i<nodes.length();i++) {
            org.json.JSONObject node=nodes.optJSONObject(i);
            if(node!=null && xqBounds(node).contains((int)event.getX(),(int)event.getY())) { hovered=node.optInt("id");break; }
        }
        if(hovered!=xqHovered) {
            if(hovered!=-1)xqEvent(hovered,android.view.accessibility.AccessibilityEvent.TYPE_VIEW_HOVER_ENTER);
            if(xqHovered!=-1)xqEvent(xqHovered,android.view.accessibility.AccessibilityEvent.TYPE_VIEW_HOVER_EXIT);
            xqHovered=hovered;
        }
        return true;
    }
    @Override public android.view.accessibility.AccessibilityNodeProvider getAccessibilityNodeProvider() {
        return new android.view.accessibility.AccessibilityNodeProvider() {
            @Override public android.view.accessibility.AccessibilityNodeInfo createAccessibilityNodeInfo(int id) {
                android.view.accessibility.AccessibilityNodeInfo info=android.view.accessibility.AccessibilityNodeInfo.obtain();
                info.setPackageName(getContext().getPackageName());
                if(id==View.NO_ID) {
                    QuadSurface.this.onInitializeAccessibilityNodeInfo(info);
                    info.setSource(QuadSurface.this);info.setClassName("android.view.ViewGroup");
                    info.setEnabled(true);info.setVisibleToUser(isShown());
                    android.graphics.Rect hostBounds=new android.graphics.Rect(0,0,getWidth(),getHeight());
                    info.setBoundsInParent(hostBounds);int[] hostXY=new int[2];getLocationOnScreen(hostXY);hostBounds.offset(hostXY[0],hostXY[1]);info.setBoundsInScreen(hostBounds);
                    info.setContentDescription(xqTree.optString("summary"));
                    org.json.JSONArray nodes=xqTree.optJSONArray("elements");
                    if(nodes!=null)for(int i=0;i<nodes.length();i++)info.addChild(QuadSurface.this,nodes.optJSONObject(i).optInt("id"));
                    return info;
                }
                org.json.JSONObject node=xqElement(id);if(node==null)return null;
                info.setSource(QuadSurface.this,id);info.setParent(QuadSurface.this);
                boolean editable=!node.isNull("value");
                info.setClassName(editable?"android.widget.EditText":node.optBoolean("control",node.optBoolean("actionable"))?"android.widget.Button":"android.widget.TextView");
                if(editable){info.setEditable(true);info.setText(node.optString("value"));info.addAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_SET_TEXT);}
                info.setContentDescription(node.optString("label")); info.setEnabled(!node.optBoolean("disabled")); info.setFocusable(true);info.setVisibleToUser(isShown());
                android.graphics.Rect bounds=xqBounds(node);info.setBoundsInParent(bounds);
                int[] xy=new int[2];getLocationOnScreen(xy);bounds.offset(xy[0],xy[1]);info.setBoundsInScreen(bounds);
                info.setAccessibilityFocused(id==xqFocused);
                info.addAction(id==xqFocused?android.view.accessibility.AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS:android.view.accessibility.AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS);
                if(node.optBoolean("actionable")){info.setClickable(true);info.addAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK);}
                return info;
            }
            @Override public boolean performAction(int id,int action,Bundle args) {
                org.json.JSONObject node=xqElement(id);if(node==null)return false;
                if(action==android.view.accessibility.AccessibilityNodeInfo.ACTION_SET_TEXT && !node.isNull("value") && args!=null) {
                    CharSequence text=args.getCharSequence(android.view.accessibility.AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE);
                    if(text==null)return false;
                    String value=text.toString();int count=value.codePointCount(0,value.length());
                    QuadNative.xiangqiNameEdit(value,count,count);return true;
                }
                if(action==android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK && node.optBoolean("actionable")) {QuadNative.xiangqiAccessibilityActivate(id);return true;}
                if(action==android.view.accessibility.AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS) {
                    if(xqFocused!=-1)xqEvent(xqFocused,android.view.accessibility.AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);
                    xqFocused=id;xqEvent(id,android.view.accessibility.AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED);return true;
                }
                if(action==android.view.accessibility.AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS && xqFocused==id) {xqFocused=-1;xqEvent(id,android.view.accessibility.AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);return true;}
                return false;
            }
        };
    }
'''
    source = source.replace(marker, body + "\n" + marker)
    marker = "    //% MAIN_ACTIVITY_BODY"
    if source.count(marker) != 1:
        raise ValueError("activity accessibility insertion point changed")
    return source.replace(marker, marker + '''
    public void xiangqiAccessibility(String json) { runOnUiThread(() -> view.xiangqiUpdateAccessibility(json)); }
''')
