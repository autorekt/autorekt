@echo off
rem Installs autorekt on Windows from CMD:
rem curl -fsSL {{SITE}}/install.cmd -o install.cmd && install.cmd && del install.cmd
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm {{SITE}}/install.ps1 | iex"
