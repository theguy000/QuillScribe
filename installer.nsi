; QuillScribe Installer Script
; Created with NSIS (Nullsoft Scriptable Install System)
; Modern UI and professional installer for QuillScribe

;--------------------------------
; Include Modern UI
!include "MUI2.nsh"
!include "FileFunc.nsh"

;--------------------------------
; General Configuration
Name "QuillScribe"
OutFile "QuillScribe-Installer.exe"
Unicode True

; Default installation directory
InstallDir "$PROGRAMFILES64\QuillScribe"

; Get installation folder from registry if available
InstallDirRegKey HKCU "Software\QuillScribe" ""

; Request application privileges for Windows Vista/7/8/10/11
RequestExecutionLevel admin

;--------------------------------
; Variables
Var StartMenuFolder

;--------------------------------
; Interface Configuration
!define MUI_ABORTWARNING
; Note: Icon files must be in ICO format for NSIS
; !define MUI_ICON "icon.ico"
; !define MUI_UNICON "icon.ico"

; Header image (optional - create a 150x57 BMP)
; !define MUI_HEADERIMAGE
; !define MUI_HEADERIMAGE_BITMAP "header.bmp"

; Welcome page image (optional - create a 164x314 BMP)
; !define MUI_WELCOMEFINISHPAGE_BITMAP "welcome.bmp"

;--------------------------------
; Pages

; Installer pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY

; Start Menu Folder Page Configuration
!define MUI_STARTMENUPAGE_REGISTRY_ROOT "HKCU" 
!define MUI_STARTMENUPAGE_REGISTRY_KEY "Software\QuillScribe" 
!define MUI_STARTMENUPAGE_REGISTRY_VALUENAME "Start Menu Folder"
!insertmacro MUI_PAGE_STARTMENU Application $StartMenuFolder

!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; Uninstaller pages
!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

;--------------------------------
; Languages
!insertmacro MUI_LANGUAGE "English"

;--------------------------------
; Version Information
VIProductVersion "1.0.0.0"
VIAddVersionKey "ProductName" "QuillScribe"
VIAddVersionKey "CompanyName" "QuillScribe Team"
VIAddVersionKey "FileDescription" "Beautiful Voice-to-Text Transcription App"
VIAddVersionKey "FileVersion" "1.0.0"
VIAddVersionKey "ProductVersion" "1.0.0"
VIAddVersionKey "LegalCopyright" "© 2024 QuillScribe Team"

;--------------------------------
; Installer Sections

Section "QuillScribe Application" SecMain
    SectionIn RO  ; Read-only section (always installed)
    
    ; Set output path to the installation directory
    SetOutPath "$INSTDIR"
    
    ; Install main application files
    File "dist\QuillScribe.exe"
    
    ; Store installation folder
    WriteRegStr HKCU "Software\QuillScribe" "" $INSTDIR
    
    ; Create uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    
    ; Add to Add/Remove Programs
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuillScribe" \
                     "DisplayName" "QuillScribe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuillScribe" \
                     "UninstallString" "$INSTDIR\Uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuillScribe" \
                     "DisplayIcon" "$INSTDIR\QuillScribe.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuillScribe" \
                     "Publisher" "QuillScribe Team"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuillScribe" \
                     "DisplayVersion" "1.0.0"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuillScribe" \
                      "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuillScribe" \
                      "NoRepair" 1
    
    ; Calculate and store installation size
    ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuillScribe" \
                      "EstimatedSize" "$0"
SectionEnd

Section "Desktop Shortcut" SecDesktop
    CreateShortcut "$DESKTOP\QuillScribe.lnk" "$INSTDIR\QuillScribe.exe" "" \
                   "$INSTDIR\QuillScribe.exe" 0 SW_SHOWNORMAL \
                   "" "Beautiful Voice-to-Text Transcription"
SectionEnd

Section "Quick Launch Shortcut" SecQuickLaunch
    CreateShortcut "$QUICKLAUNCH\QuillScribe.lnk" "$INSTDIR\QuillScribe.exe" "" \
                   "$INSTDIR\QuillScribe.exe" 0 SW_SHOWNORMAL \
                   "" "Beautiful Voice-to-Text Transcription"
SectionEnd

Section /o "Auto-start with Windows" SecAutoStart
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" \
                     "QuillScribe" "$INSTDIR\QuillScribe.exe"
SectionEnd

; Start Menu shortcuts
Section "Start Menu Shortcuts" SecStartMenu
    !insertmacro MUI_STARTMENU_WRITE_BEGIN Application
        
        CreateDirectory "$SMPROGRAMS\$StartMenuFolder"
        CreateShortcut "$SMPROGRAMS\$StartMenuFolder\QuillScribe.lnk" \
                       "$INSTDIR\QuillScribe.exe" "" \
                       "$INSTDIR\QuillScribe.exe" 0 SW_SHOWNORMAL \
                       "" "Beautiful Voice-to-Text Transcription"
        CreateShortcut "$SMPROGRAMS\$StartMenuFolder\Uninstall QuillScribe.lnk" \
                       "$INSTDIR\Uninstall.exe"
        
    !insertmacro MUI_STARTMENU_WRITE_END
SectionEnd

;--------------------------------
; Section Descriptions
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
    !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "The main QuillScribe application files (required)"
    !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Create a desktop shortcut for QuillScribe"
    !insertmacro MUI_DESCRIPTION_TEXT ${SecQuickLaunch} "Create a quick launch shortcut for QuillScribe"
    !insertmacro MUI_DESCRIPTION_TEXT ${SecAutoStart} "Automatically start QuillScribe when Windows starts"
    !insertmacro MUI_DESCRIPTION_TEXT ${SecStartMenu} "Create Start Menu shortcuts"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

;--------------------------------
; Functions

Function .onInit
    ; Check if application is already running
    System::Call 'kernel32::CreateMutex(i 0, i 0, t "QuillScribeInstaller") i .r1 ?e'
    Pop $0
    StrCmp $0 0 +3
        MessageBox MB_OK|MB_ICONEXCLAMATION "The installer is already running."
        Abort
        
    ; Note: Windows version check removed for compatibility
    ; QuillScribe should work on Windows 10 and later
FunctionEnd

Function .onInstSuccess
    MessageBox MB_YESNO|MB_ICONQUESTION "Installation completed successfully!$\n$\nDo you want to launch QuillScribe now?" IDNO +2
        Exec "$INSTDIR\QuillScribe.exe"
FunctionEnd

;--------------------------------
; Uninstaller Section

Section "Uninstall"
    ; Remove registry keys
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuillScribe"
    DeleteRegKey HKCU "Software\QuillScribe"
    
    ; Remove auto-start entry
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "QuillScribe"
    
    ; Remove Start Menu shortcuts
    !insertmacro MUI_STARTMENU_GETFOLDER Application $StartMenuFolder
    Delete "$SMPROGRAMS\$StartMenuFolder\QuillScribe.lnk"
    Delete "$SMPROGRAMS\$StartMenuFolder\Uninstall QuillScribe.lnk"
    RMDir "$SMPROGRAMS\$StartMenuFolder"
    
    ; Remove shortcuts
    Delete "$DESKTOP\QuillScribe.lnk"
    Delete "$QUICKLAUNCH\QuillScribe.lnk"
    
    ; Remove application files
    RMDir /r "$INSTDIR"
    
SectionEnd

Function un.onInit
    MessageBox MB_YESNO|MB_ICONQUESTION "Are you sure you want to completely remove QuillScribe and all of its components?" IDYES +2
        Abort
FunctionEnd

Function un.onUninstSuccess
    MessageBox MB_OK|MB_ICONINFORMATION "QuillScribe has been successfully removed from your computer."
FunctionEnd
