# The expansion specifier is in an included file, so a diagnostic inside the
# expansion reports where the specifier is included

module template T() {
  constant k = undefinedX
}

include "expand_in_include.fppi"
