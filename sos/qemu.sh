#!/bin/bash

BOOT_IMAGE="$(realpath "$1")"
cd /mnt/c
"/mnt/c/Program Files/qemu/qemu-system-x86_64.exe" -drive format=raw,file='//wsl$/Ubuntu'"${BOOT_IMAGE}" ${@:2}