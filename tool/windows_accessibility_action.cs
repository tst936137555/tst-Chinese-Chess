using System;
using System.Diagnostics;
using System.Windows.Automation;
public static class AccessibilityAction {
    public static int Main(string[] args) {
        try {
            var window = AutomationElement.FromHandle(Process.GetProcessById(int.Parse(args[0])).MainWindowHandle);
            var control = window.FindFirst(TreeScope.Descendants, new PropertyCondition(AutomationElement.NameProperty, args[1]));
            if (control == null || !control.Current.IsEnabled) throw new Exception("Control missing or disabled: " + args[1]);
            if (args.Length < 3 || args[2] == "focus") control.SetFocus();
            else ((InvokePattern)control.GetCurrentPattern(InvokePattern.Pattern)).Invoke();
            return 0;
        } catch (Exception error) { Console.Error.WriteLine(error); return 1; }
    }
}
