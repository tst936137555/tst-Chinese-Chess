"""Run XCTest system touch/rotation against the already built, installed simulator app."""
from pathlib import Path
import json
import subprocess

ROOT=Path(__file__).resolve().parent.parent

def main():
    devices=json.loads(subprocess.check_output(["xcrun","simctl","list","devices","booted","--json"]))["devices"]
    uid=next(d["udid"] for ds in devices.values() for d in ds if d["state"]=="Booted" and "iPhone" in d["name"])
    root=ROOT/"target/ios-native-input"
    project=root/"NativeInput.xcodeproj"
    schemes=project/"xcshareddata/xcschemes"
    schemes.mkdir(parents=True,exist_ok=True)
    source=str(ROOT/"tests/ios/NativeInput.swift").replace("\\","/").replace('"','\\"')
    pbx='''// !$*UTF8*$!
{ archiveVersion=1; classes={}; objectVersion=56; objects={
 A00000000000000000000001={isa=PBXProject; buildConfigurationList=A00000000000000000000002; compatibilityVersion="Xcode 14.0"; mainGroup=A00000000000000000000003; productRefGroup=A00000000000000000000004; projectDirPath=""; targets=(A00000000000000000000005,); };
 A00000000000000000000002={isa=XCConfigurationList; buildConfigurations=(A00000000000000000000006,); defaultConfigurationIsVisible=0; defaultConfigurationName=Debug;};
 A00000000000000000000003={isa=PBXGroup; children=(A00000000000000000000007,A00000000000000000000004,); sourceTree="<group>";};
 A00000000000000000000004={isa=PBXGroup; children=(A00000000000000000000008,); name=Products; sourceTree="<group>";};
 A00000000000000000000005={isa=PBXNativeTarget; buildConfigurationList=A00000000000000000000009; buildPhases=(A00000000000000000000010,); buildRules=(); dependencies=(); name=NativeInput; productName=NativeInput; productReference=A00000000000000000000008; productType="com.apple.product-type.bundle.ui-testing";};
 A00000000000000000000006={isa=XCBuildConfiguration; buildSettings={SDKROOT=iphonesimulator; IPHONEOS_DEPLOYMENT_TARGET=15.0; SWIFT_VERSION=5.0;}; name=Debug;};
 A00000000000000000000007={isa=PBXFileReference; lastKnownFileType=sourcecode.swift; path="SOURCE_PATH"; sourceTree="<absolute>";};
 A00000000000000000000008={isa=PBXFileReference; explicitFileType=wrapper.cfbundle; path=NativeInput.xctest; sourceTree=BUILT_PRODUCTS_DIR;};
 A00000000000000000000009={isa=XCConfigurationList; buildConfigurations=(A00000000000000000000011,); defaultConfigurationIsVisible=0; defaultConfigurationName=Debug;};
 A00000000000000000000010={isa=PBXSourcesBuildPhase; buildActionMask=2147483647; files=(A00000000000000000000012,); runOnlyForDeploymentPostprocessing=0;};
 A00000000000000000000011={isa=XCBuildConfiguration; buildSettings={PRODUCT_BUNDLE_IDENTIFIER="com.tst.xiangqi.native-input-tests"; PRODUCT_NAME="$(TARGET_NAME)"; GENERATE_INFOPLIST_FILE=YES; ENABLE_TESTING_SEARCH_PATHS=YES; TARGETED_DEVICE_FAMILY=1; SUPPORTED_PLATFORMS=iphonesimulator;}; name=Debug;};
 A00000000000000000000012={isa=PBXBuildFile; fileRef=A00000000000000000000007;};
}; rootObject=A00000000000000000000001; }
'''
    (project/"project.pbxproj").write_text(pbx.replace("SOURCE_PATH",source),encoding="utf-8")
    (schemes/"NativeInput.xcscheme").write_text('''<?xml version="1.0" encoding="UTF-8"?>
<Scheme LastUpgradeVersion="1600" version="1.3"><BuildAction parallelizeBuildables="NO" buildImplicitDependencies="YES"><BuildActionEntries><BuildActionEntry buildForTesting="YES" buildForRunning="NO" buildForProfiling="NO" buildForArchiving="NO" buildForAnalyzing="NO"><BuildableReference BuildableIdentifier="primary" BlueprintIdentifier="A00000000000000000000005" BuildableName="NativeInput.xctest" BlueprintName="NativeInput" ReferencedContainer="container:NativeInput.xcodeproj"/></BuildActionEntry></BuildActionEntries></BuildAction><TestAction buildConfiguration="Debug" selectedDebuggerIdentifier="Xcode.DebuggerFoundation.Debugger.LLDB" selectedLauncherIdentifier="Xcode.IDEFoundation.Launcher.LLDB" shouldUseLaunchSchemeArgsEnv="YES"><Testables><TestableReference skipped="NO"><BuildableReference BuildableIdentifier="primary" BlueprintIdentifier="A00000000000000000000005" BuildableName="NativeInput.xctest" BlueprintName="NativeInput" ReferencedContainer="container:NativeInput.xcodeproj"/></TestableReference></Testables></TestAction></Scheme>
''',encoding="utf-8")
    result=ROOT/"target/ios-smoke/native-input.xcresult"
    subprocess.run(["xcodebuild","test","-project",str(project),"-scheme","NativeInput","-destination",f"id={uid}",
                    "-parallel-testing-enabled","NO","-resultBundlePath",str(result),"CODE_SIGN_IDENTITY=-"],check=True,timeout=420)

if __name__=="__main__":main()
