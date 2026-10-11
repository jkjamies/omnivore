#!/bin/sh
# Start as root only long enough to make /data writable by the omnivore user,
# then drop privileges before the server runs.
#
# Images before 0.1.0 ran as root, so an existing volume or bind mount holds a
# root-owned omnivore.db that UID 10001 cannot open. Re-owning it here lets an
# upgrade start without a manual chown. Started with `--user`, the container is
# already unprivileged and this is skipped.
set -e

if [ "$(id -u)" = "0" ]; then
    if ! find /data \! -user omnivore -exec chown omnivore:omnivore {} + 2>/dev/null; then
        echo "warning: could not chown /data to omnivore (UID 10001); the server may be unable to write it" >&2
    fi
    exec setpriv --reuid=omnivore --regid=omnivore --init-groups -- "$@"
fi

exec "$@"
