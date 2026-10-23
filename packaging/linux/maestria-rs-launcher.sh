#!/bin/sh
if [ "${MAESTRIA_IBUS:-0}" != 1 ]; then
  export MAESTRIA_SAVED_XMODIFIERS="${XMODIFIERS:-}"
  export XMODIFIERS=
fi
export MAESTRIA_APP_ID=br.com.marrow.maestria-rs
exec /opt/maestria-rs/maestria-rs "$@"
