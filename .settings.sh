if [[ -x $(which sqlx 2>/dev/null) ]]
then
  . <(sqlx completions bash)
else
  echo "sqlx is not installed; install with cargo install sqlx-cli"
fi

PATH=$(pwd)/bin:$PATH

export PGHOST=localhost
export PGPORT=5555

export DATABASE_URL="postgres://skjera:skjera@localhost/skjera"
