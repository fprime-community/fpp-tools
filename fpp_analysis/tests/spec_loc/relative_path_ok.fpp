# Location paths are normalized: `.` and `..` components are resolved
locate constant a at "./relative_path_ok.fpp"
locate constant b at "sub/../relative_path_ok.fpp"
locate constant c at "../spec_loc/relative_path_ok.fpp"

constant a = 0
constant b = 1
constant c = 2
