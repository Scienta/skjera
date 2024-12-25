if [[ -x $(which sqlx 2>/dev/null) ]]
then
  . <(sqlx completions bash)
else
  echo "sqlx is not installed; install with cargo install sqlx-cli"
fi

export PGHOST=localhost
export PGPORT=5555
export PGUSER=postgres
export PGPASSWORD=postgres

export DATABASE_URL="postgres://skjera-owner:skjera-owner@localhost/skjera"
