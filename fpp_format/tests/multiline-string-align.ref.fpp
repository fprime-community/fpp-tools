module DataProducts {
    instance dpCat: Svc.DpCatalog base id 0x00000 \
        queue size 10 {
            phase Fpp.ToCpp.Phases.configComponents """
        Fw::FileNameString dpDir(DataProductsConfig::Paths::dpDir);
        DataProducts::dpCat.configure(&dpDir,1,dpState,0, DataProducts::Allocation::memAllocator);
        """
        }

    topology Subtopology {
        instance dpCat
        instance dpMgr

        connections DataProducts {
            dpMgr.bufferGetOut[0]   -> dpCat.bufferGetCallee
            dpMgr.productSendOut[0] -> dpCat.bufferSendInFill
            dpCat.dpWrittenOut      -> dpCat.addToCat
        }

        @ Input port for get requests
        port productGetIn = dpMgr.productGetIn
        @ Output port for responses
        port productResponseOut = dpMgr.productResponseOut
    }
}
