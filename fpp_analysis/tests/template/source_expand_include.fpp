# An include specifier in an expansion written in the source is resolved, and a
# diagnostic inside it reports where the file is included

module template T {

}

expand T {

  include "source_expand_include.fppi"

}
