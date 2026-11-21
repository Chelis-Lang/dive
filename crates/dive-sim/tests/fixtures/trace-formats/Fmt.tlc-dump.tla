---- MODULE trace_0 ----
LOCAL INSTANCE TLC

CounterExample == 
	[action |-> {<<<<1, [x |-> 0, doubled |-> 0]>>, [name |-> "Inc", location |-> [beginLine |-> 5, beginColumn |-> 8, endLine |-> 5, endColumn |-> 53, module |-> "Fmt"]], <<2, [x |-> 1, doubled |-> 2]>>>>, <<<<2, [x |-> 1, doubled |-> 2]>>, [name |-> "Inc", location |-> [beginLine |-> 5, beginColumn |-> 8, endLine |-> 5, endColumn |-> 53, module |-> "Fmt"]], <<3, [x |-> 2, doubled |-> 4]>>>>, <<<<3, [x |-> 2, doubled |-> 4]>>, [name |-> "Inc", location |-> [beginLine |-> 5, beginColumn |-> 8, endLine |-> 5, endColumn |-> 53, module |-> "Fmt"]], <<4, [x |-> 3, doubled |-> 6]>>>>}, state |-> {<<1, [x |-> 0, doubled |-> 0]>>, <<2, [x |-> 1, doubled |-> 2]>>, <<3, [x |-> 2, doubled |-> 4]>>, <<4, [x |-> 3, doubled |-> 6]>>}]

====