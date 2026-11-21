import java.math.BigInteger;
import java.util.Arrays;
import java.util.TreeMap;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.LongAdder;
import tla2sany.semantic.SemanticNode;
import tlc2.TLC;
import tlc2.TLCGlobals;
import tlc2.output.EC;
import tlc2.tool.Action;
import tlc2.tool.ITool;
import tlc2.tool.TLCState;
import tlc2.util.BitVector;
import tlc2.util.IStateWriter;
import tlc2.value.IMVPerm;
import tlc2.value.IValue;
import util.SimpleFilenameToStream;

/** Count symmetry orbit sizes at TLC's successful state admissions. */
public final class OrbitCensus implements IStateWriter {
    private final ConcurrentHashMap<Integer, LongAdder> counts = new ConcurrentHashMap<>();
    private volatile IMVPerm[] permutations;

    private synchronized IMVPerm[] loadGroup() {
        if (permutations == null) {
            ITool tool = TLCGlobals.mainChecker.tool;
            if (tool.getViewSpec() != null) {
                throw new IllegalArgumentException("a VIEW can merge multiple state orbits");
            }
            IMVPerm[] group = tool.getSymmetryPerms();
            permutations = group == null ? new IMVPerm[0]
                : Arrays.stream(group).filter(p -> p.size() != 0).toArray(IMVPerm[]::new);
        }
        return permutations;
    }

    private void admit(TLCState state) {
        IMVPerm[] group = permutations;
        if (group == null) {
            group = loadGroup();
        }
        int stabilizer = 1; // identity
        for (IMVPerm permutation : group) {
            boolean fixed = true;
            for (var variable : state.getVars()) {
                IValue value = state.lookup(variable.getName());
                if (!value.equals(value.permute(permutation))) {
                    fixed = false;
                    break;
                }
            }
            if (fixed) {
                stabilizer++;
            }
        }
        int order = Math.addExact(group.length, 1);
        if (order % stabilizer != 0) {
            throw new IllegalStateException("stabilizer order must divide group order");
        }
        counts.computeIfAbsent(order / stabilizer, k -> new LongAdder()).increment();
    }

    public void writeState(TLCState state) { admit(state); }
    public void writeState(TLCState src, TLCState dst, short flags) {
        if (flags == IsUnseen) { admit(dst); }
    }
    public void writeState(TLCState src, TLCState dst, short flags, Action action) {
        writeState(src, dst, flags);
    }
    public void writeState(TLCState src, TLCState dst, short flags, Action action, SemanticNode pred) {
        writeState(src, dst, flags);
    }
    public void writeState(TLCState src, TLCState dst, short flags, Visualization kind) {
        throw new UnsupportedOperationException("liveness visualization");
    }
    public void writeState(TLCState src, TLCState dst, BitVector bits, int from, int count, short flags) {
        throw new UnsupportedOperationException("liveness graph");
    }
    public void writeState(TLCState src, TLCState dst, BitVector bits, int from, int count, short flags, Visualization kind) {
        throw new UnsupportedOperationException("liveness graph");
    }
    public void close() {}
    public void snapshot() {}
    public String getDumpFileName() { return "orbit-census"; }
    public boolean isNoop() { return false; }
    public boolean isDot() { return false; }
    public boolean isConstrained() { return false; }

    private void report() {
        var checker = TLCGlobals.mainChecker;
        long representatives = 0;
        BigInteger labeled = BigInteger.ZERO;
        var histogram = new TreeMap<Integer, Long>();
        for (var entry : counts.entrySet()) {
            long count = entry.getValue().sum();
            representatives = Math.addExact(representatives, count);
            labeled = labeled.add(BigInteger.valueOf(count).multiply(BigInteger.valueOf(entry.getKey())));
            histogram.put(entry.getKey(), count);
        }
        if (checker.getStateQueueSize() != 0 || representatives != checker.getDistinctStatesGenerated()) {
            throw new IllegalStateException("incomplete state admissions");
        }
        System.out.printf("ORBIT_CENSUS representatives=%d labeled=%s histogram=%s%n", representatives, labeled, histogram);
    }

    public static void main(String[] args) {
        if (args.length != 4) {
            throw new IllegalArgumentException("usage: OrbitCensus MODULE CFG WORKERS NEW_METADIR");
        }
        var tlc = new TLC();
        String[] options = {"-workers", args[2], "-seed", "0", "-fp", "0", "-checkpoint", "0",
                            "-metadir", args[3], "-config", args[1], args[0]};
        if (!tlc.handleParameters(options)) { System.exit(1); }
        tlc.setResolver(new SimpleFilenameToStream());
        var census = new OrbitCensus();
        tlc.setStateWriter(census);
        int code = tlc.process();
        if (code == EC.NO_ERROR) { census.report(); }
        System.exit(EC.ExitStatus.errorConstantToExitStatus(code));
    }
}
