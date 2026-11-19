# s7-env.sh: sourced by the S7 chains; the pinned oracle JRE and jar from dive-conformance oracle fetch.
export JAVA=$(ls -d ~/.cache/dive/jre/*/jdk*/bin/java | head -1)
export TLA2TOOLS=$(ls ~/.cache/dive/sha256/*/tla2tools.jar | head -1)
export OUT=~/st PATH=/usr/local/bin:/usr/bin:/bin
