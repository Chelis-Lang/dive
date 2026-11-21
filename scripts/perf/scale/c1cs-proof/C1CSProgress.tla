---- MODULE C1CSProgress ----
EXTENDS Integers, FiniteSets
VARIABLES
    \* @type: Int -> Str;
    pc,
    \* @type: Int -> VAL;
    v,
    \* @type: Int -> VAL;
    dValue,
    \* @type: Set({type: Str, value: VAL, sndr: Int});
    bcastMsg,
    \* @type: Int -> Set({type: Str, value: VAL, sndr: Int});
    rcvdMsg

N == 4
T == 1
F == 1
Values == {"v1_OF_VAL", "v2_OF_VAL"}
Bottom == "Bottom_OF_VAL"
INSTANCE C1CSEnabled

DecisionMsgs == {m \in bcastMsg : m.type = "Decision"}
ProposedBy(i) == {m \in bcastMsg : m.type = "Proposed" /\ m.sndr = i}
Inv == /\ TypeOK
       /\ \A i \in Proc: pc[i] # "CRASH"
       /\ \A i \in Proc: pc[i] = "PROPOSE" \/ ProposedBy(i) # {} \/ DecisionMsgs # {}

Level(loc) == CASE loc = "PROPOSE" -> 0 [] loc = "DECIDE" -> 1
                   [] loc = "CALL" -> 2 [] loc = "DONE" -> 3 [] OTHER -> 4
RankStep ==
    \/ vars' = vars
    \/ /\ \A i \in Proc: Level(pc'[i]) >= Level(pc[i]) /\ rcvdMsg[i] \subseteq rcvdMsg'[i]
       /\ \E i \in Proc: Level(pc'[i]) > Level(pc[i]) \/ rcvdMsg[i] # rcvdMsg'[i]

Progress == Goal \/ FairEnabled
FairNonstutter == vars' # vars
SafetyAll == Inv /\ IndStrengthens /\ Validity /\ Agreement /\ WeakAgreement
Support(x) == Cardinality({m \in bcastMsg : m.type = "Proposed" /\ m.value = x})
QuorumSupport ==
    /\ \A i \in Proc: pc[i] = "CALL" => Support(v[i]) >= N - 2*T
    /\ \A m \in bcastMsg:
           (m.type = "Proposed" /\ v[m.sndr] # m.value) => Support(v[m.sndr]) >= N - 2*T
StrongSafety == SafetyAll /\ QuorumSupport
====
