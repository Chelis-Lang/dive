import java.nio.file.Files;
import java.nio.file.Path;
import tla2sany.output.SilentSanyOutput;
import tla2sany.parser.TLAplusParser;

/** Invoke the pinned grammar directly, without module or PlusCal validation. */
public final class DiveSanyParse {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("one module path required");
        }
        Path path = Path.of(args[0]);
        TLAplusParser parser = new TLAplusParser(new SilentSanyOutput(), Files.readAllBytes(path));
        String verdict = "reject";
        if (parser.parse()) {
            String filename = path.getFileName().toString();
            String stem = filename.substring(0, filename.length() - ".tla".length());
            verdict = parser.moduleName().equals(stem) ? "accept" : "name-mismatch";
        }
        System.out.println("DIVE-SANY-PARSE " + verdict);
    }
}
