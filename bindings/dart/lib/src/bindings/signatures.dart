// ignore_for_file: camel_case_types, non_constant_identifier_names

import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'types.dart';

typedef open_engine_with_path_native_t = Int32 Function(
    Pointer<Utf8> path, Pointer<Pointer<Engine>> engine_out);

typedef source_meta_native_t = Int32 Function(
    Pointer<Engine> engine, Pointer<Pointer<Utf8>> out);
