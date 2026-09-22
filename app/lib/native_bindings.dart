@DefaultAsset('package:pixelstrict/native_bindings.dart')
library;

import 'dart:ffi';

@Native<Pointer<Uint8> Function(Size)>(symbol: 'ps_alloc')
external Pointer<Uint8> allocate(int length);
@Native<Void Function(Pointer<Uint8>, Size)>(symbol: 'ps_dealloc')
external void deallocate(Pointer<Uint8> pointer, int length);
@Native<Pointer<Void> Function(Pointer<Uint8>, Size, Pointer<Uint8>, Size)>(
  symbol: 'ps_convert',
)
external Pointer<Void> convertNative(
  Pointer<Uint8> image,
  int length,
  Pointer<Uint8> options,
  int optionsLength,
);
@Native<Pointer<Uint8> Function(Pointer<Void>)>(symbol: 'ps_result_json')
external Pointer<Uint8> resultJson(Pointer<Void> result);
@Native<Size Function(Pointer<Void>)>(symbol: 'ps_result_json_len')
external int resultJsonLength(Pointer<Void> result);
@Native<Pointer<Uint8> Function(Pointer<Void>)>(symbol: 'ps_result_png')
external Pointer<Uint8> resultPng(Pointer<Void> result);
@Native<Size Function(Pointer<Void>)>(symbol: 'ps_result_png_len')
external int resultPngLength(Pointer<Void> result);
@Native<Void Function(Pointer<Void>)>(symbol: 'ps_result_free')
external void freeResult(Pointer<Void> result);
