@ Implements lists separated by ends of line
module M {
    topology A implements I, J {
    }

    topology B implements I, J, K {
    }

    topology C implements I, # first
    J {
    }

    topology D implements I, J, {
    }
}
