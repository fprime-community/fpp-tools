topology T implements I { }
interface I { import J }
interface J { import I }
