---- MODULE Lease ----
EXTENDS Naturals
CONSTANT NoLease
VARIABLES lease, latest
Init == lease = NoLease /\ latest = [log |-> <<NoLease>>, counter |-> 1]
Next == UNCHANGED <<lease, latest>>
Response == /\ (lease /= NoLease) ~> (lease.counter < 2)
            /\ <>(latest /= <<NoLease>>)
DistinctDomains == latest /= <<NoLease>>
====
