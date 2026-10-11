# An absolute template name is looked up in the global scope only
module N {
  module template T { constant c = 0 }
  expand .T
}
