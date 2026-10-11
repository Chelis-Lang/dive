# Corrected actual integer-return observer

This is a separate twelve-row controlled reference epoch. The original
claimed-queue-controlled3 adapter called `c.modelCheck()` but discarded its
integer result. Its documentation incorrectly called that method void. Preserve
those original source/raw files as development evidence. The corrected adapter
records the actual returned int separately from the reflected `errorCode`,
observer process exit, and unmodified TLC CLI exit.

The pinned `AbstractChecker.modelCheck` is `final public int`; it returns
nonzero `modelCheckImpl` result or otherwise its `errorCode`. Actual widths
1/4/8 give TRUE=0, FALSE=2104, division=1000, and temporal violation=2116. The
observer subprocess exits zero because its main method does not forward the
integer return to `System.exit`. This does not establish a Java bug or native
ordinary scheduling/model proof.

Instrumentation holds a genuinely dequeued state with a ten-second bounded
latch until actual queue suspension entry; it delegates actual queue and live
methods without replacing their results. Cleanup events after method return
are observer cleanup. These are controlled timing observations, separate from
natural unmodified CLI runs.
