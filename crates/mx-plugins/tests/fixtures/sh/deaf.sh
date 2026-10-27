#!/bin/sh
trap '' TERM
while IFS= read -r line; do :; done
exec sleep 30
