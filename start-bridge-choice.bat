@echo off
title Stratum Bridge Launcher
echo.
echo ========================================
echo      Stratum Bridge Launcher
echo ========================================
echo.
echo 1. Start in IN-PROCESS mode
echo 2. Start in EXTERNAL mode
echo 3. Exit
echo.
set /p choice="Select mode (1-3): "

if "%choice%"=="1" (
    echo.
    echo Starting stratum-bridge in IN-PROCESS mode...
    stratum-bridge.exe --config config.yaml --node-mode inprocess --node-args="--utxoindex --rpclisten=127.0.0.1:16110 --rpclisten-borsh=127.0.0.1:17110 --disable-upnp"
) else if "%choice%"=="2" (
    echo.
    echo Starting stratum-bridge in EXTERNAL mode...
    stratum-bridge.exe --config config.yaml --node-mode external
) else if "%choice%"=="3" (
    echo Exiting...
    exit /b
) else (
    echo Invalid choice. Please run again.
)

pause
