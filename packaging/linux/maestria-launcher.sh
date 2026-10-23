#!/bin/sh
if [ "${MAESTRIA_IBUS:-0}" != 1 ]; then
  export MAESTRIA_SAVED_XMODIFIERS="${XMODIFIERS:-}"
  export XMODIFIERS=
fi
exec /opt/maestria/maestria "$@"
