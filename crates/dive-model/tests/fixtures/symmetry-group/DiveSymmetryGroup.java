// Original MIT test driver: calls the pinned TLC kernel, without model binding.
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import tlc2.value.IMVPerm;
import tlc2.value.impl.*;

public final class DiveSymmetryGroup {
    public static void main(String[] args) throws Exception {
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            String[] fields = line.split("\\|", -1);
            int n = Integer.parseInt(fields[1]);
            ModelValue.init();
            Value[] models = new Value[n];
            for (int i = 0; i < n; i++) models[i] = ModelValue.make("m" + i);
            ModelValue.setValues();
            String[] declarations = fields[2].isEmpty() ? new String[0] : fields[2].split(";", -1);
            Value[] functions = new Value[declarations.length];
            for (int g = 0; g < functions.length; g++) {
                String[] pairs = declarations[g].isEmpty() ? new String[0] : declarations[g].split(",");
                Value[] domain = new Value[pairs.length];
                Value[] range = new Value[pairs.length];
                for (int i = 0; i < pairs.length; i++) {
                    String[] pair = pairs[i].split(":");
                    domain[i] = models[Integer.parseInt(pair[0])];
                    range[i] = models[Integer.parseInt(pair[1])];
                }
                functions[g] = new FcnRcdValue(domain, range, true);
            }
            // Preserve the supplied enumeration order to isolate the closure
            // kernel. Set normalization and Tool.getSymmetryPerms are not tested.
            IMVPerm[] group;
            try {
                group = MVPerms.permutationSubgroup(new SetEnumValue(functions, true));
            } catch (IllegalArgumentException error) {
                if (functions.length >= 2) throw error;
                System.out.println("{\"id\":\"" + fields[0] + "\",\"error\":\"too-few-generators\"}");
                continue;
            }
            StringBuilder out = new StringBuilder("{\"id\":\"").append(fields[0])
                .append("\",\"maps\":[");
            for (int g = 0; g < group.length; g++) {
                if (g > 0) out.append(',');
                int[] images = new int[n];
                for (int i = 0; i < n; i++) {
                    Value image = (Value) group[g].get(models[i]);
                    images[i] = image == null ? i : ((ModelValue) image).index;
                }
                out.append(Arrays.toString(images));
            }
            System.out.println(out.append("]}"));
        }
    }
}
