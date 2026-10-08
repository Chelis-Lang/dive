import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Method;
import java.math.BigDecimal;
import java.math.MathContext;

/**
 * Reads lines "d <hex-bits>" (a double) or "g <long>" (a minimum fingerprint gap) and
 * prints TLC's renderings, using TLC's own private ProbabilityToString.
 */
public class ProbProbe {
    public static void main(String[] args) throws Exception {
        Class<?> c = Class.forName("tlc2.tool.AbstractChecker");
        Method m = c.getDeclaredMethod("ProbabilityToString", double.class, int.class);
        m.setAccessible(true);
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
        String line;
        StringBuilder out = new StringBuilder();
        while ((line = in.readLine()) != null) {
            String[] p = line.trim().split(" ");
            if (p[0].equals("d")) {
                double x = Double.longBitsToDouble(Long.parseUnsignedLong(p[1], 16));
                out.append(p[1]).append('\t').append(Double.toString(x)).append('\t')
                   .append((String) m.invoke(null, x, 2)).append('\n');
            } else if (p[0].equals("o")) {
                long d = Long.parseLong(p[1]);
                long g = Long.parseLong(p[2]);
                double x = tlc2.tool.AbstractChecker.calculateOptimisticProbability(d, g);
                out.append(p[1]).append(' ').append(p[2]).append('\t')
                   .append((String) m.invoke(null, x, 2)).append('\n');
            } else {
                long g = Long.parseLong(p[1]);
                BigDecimal a = BigDecimal.valueOf(1d).divide(BigDecimal.valueOf(g), new MathContext(2));
                out.append(p[1]).append('\t').append((String) m.invoke(null, a.doubleValue(), 2)).append('\n');
            }
        }
        System.out.print(out);
    }
}
