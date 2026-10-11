# SLATECORE LAUNCHER by GENSLATE

Version {{version}}

SLATECORE LAUNCHER is a portable app launcher for Windows. It lists the GENSLATE apps, the
PortableApps.com apps and the portapps.io apps that sit next to it, and starts them with one
keystroke. It lives on a USB drive or SSD: plug it in, use it, unplug it.

## Start it

1. Unzip the whole package onto a drive, into a folder you can write to (a folder called
   SLATECORE is a good choice). Do not run it from inside the zip, and not from a network share.
2. Start `programs\genslate\launcher\slatecore-launcher.exe`. It runs in the notification area
   (the tray); it does not install anything and never starts by itself with Windows.
3. Closing its window hides it; choose Quit in the tray menu to end it.

Windows 10 or 11 on 64-bit PCs. Start it normally, not as administrator.

## What is in the folder

- `programs\genslate\` holds GENSLATE apps, `programs\portableapps.com\` and
  `programs\portapps.io\` hold the apps you copy there.
- `other\launcher\` holds the launcher's settings (`configs`), logs, cache and database.
- `storage\users\shared\` holds your Desktop, Documents, Downloads, Music, Pictures and Videos
  folders. `storage\vault\` holds the encrypted vault: only the vault is encrypted, and a
  forgotten vault password cannot be recovered.

Everything the launcher writes goes into this folder. It does not write to the registry, to
`%APPDATA%`, `%LOCALAPPDATA%`, the Start Menu or the PC's temporary folder, and it keeps its
web view data inside the folder too. Move or copy the folder and everything moves with it.

## WebView2

The launcher draws its window with the Microsoft Edge WebView2 runtime. It uses the one that is
already on the PC (Windows 11 and most updated Windows 10 PCs have it). For a PC without it,
unzip the separate `slatecore-launcher-<version>-webview2-runtime.zip` over this folder: it adds
`other\shared\webview2\<version>-x64\`, which the launcher then uses. That runtime is Microsoft's
software, under Microsoft's license terms (a notice is in `other\launcher\licenses\`); WebView2
sends some diagnostic data to Microsoft, which the launcher cannot switch off.

## What the launcher cannot control

The launcher leaves no trace of its own on a PC, but some records are made by Windows or by other
programs, outside its control:

- Windows keeps Recent files and jump lists, and Prefetch files for programs that run.
- Windows remembers tray icon settings (the notification-area icon list).
- Apps from PortableApps.com and portapps.io, and any program the launcher starts, write whatever
  they write; some leave files or registry entries on the PC.
- WebView2 and the graphics driver may keep shader caches, crash reports and diagnostic data in
  Windows' own folders.
- Security software and Windows Error Reporting may log what ran.

If you need more than this, use a PC you trust and clear the Windows records yourself.

## More

- Documents: `other\launcher\documents\`.
- GENSLATE is the developer and publisher. SLATECORE is the suite of apps this launcher belongs to.
