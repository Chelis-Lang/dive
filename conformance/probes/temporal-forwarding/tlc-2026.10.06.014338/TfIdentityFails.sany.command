$JAVA -XX:+UseParallelGC -Xmx512m -Xss8m -Duser.language=en -Duser.country=US -Dfile.encoding=UTF-8 -cp $PINNED_TLC_JAR tla2sany.SANY -error-codes TfIdentityFails.tla
