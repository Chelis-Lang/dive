------------------------ MODULE KRaftVoteDrain ------------------------
EXTENDS kraft_kip_996

\* Include ordinary votes. They share the same per-peer fair handler with pre-votes.
Outgoing(s) == {m \in net_messages : m.type = RequestVoteRequest /\ m.source = s}
Incoming(s) == {m \in net_messages : m.type = RequestVoteResponse /\ m.dest = s}
Rank(s) == 2 * Cardinality(Outgoing(s)) + Cardinality(Incoming(s))

DrainEnabled ==
    \A s \in AllServers : IsProspective(s) =>
        /\ (Rank(s) = 0 => ENABLED <<ConcludePreVote(s)>>_vars)
        /\ \A m \in Outgoing(s) : ENABLED <<HandleVoteRequest(m.dest, s)>>_vars
        /\ \A m \in Incoming(s) : ENABLED <<HandleVoteResponse(s, m.source)>>_vars

DrainStep ==
    \A s \in AllServers :
        (IsProspective(s) /\ IsProspective(s)') =>
            /\ Rank(s)' <= Rank(s)
            /\ (Rank(s)' = Rank(s) => UNCHANGED <<Outgoing(s), Incoming(s)>>)
            /\ ~ConcludePreVote(s)
            /\ \A p \in AllServers :
                (HandleVoteRequest(p, s) \/ HandleVoteResponse(s, p)) => Rank(s)' < Rank(s)

DrainSteps == [][DrainStep]_vars

\* Reaching one of these conditions gives a finite nonvacuity witness.
NoOutgoing == ~\E s \in AllServers : IsProspective(s) /\ Outgoing(s) # {}
NoIncoming == ~\E s \in AllServers : IsProspective(s) /\ Incoming(s) # {}
NoEmpty == ~\E s \in AllServers : IsProspective(s) /\ Rank(s) = 0
Bound == /\ aux_ctrs.prevote_ctr <= 2
         /\ \A s \in StartedServers : current_epoch[s] <= 2
=======================================================================
