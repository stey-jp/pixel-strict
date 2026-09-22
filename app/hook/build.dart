import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/hooks.dart';
import 'package:native_toolchain_rust/native_toolchain_rust.dart';

void main(List<String> args) async {
  await build(args, (input, output) async {
    if (!input.config.buildCodeAssets) return;
    final code = input.config.code;
    final environment = <String, String>{};
    if (code.targetOS == OS.android) {
      // native_toolchain_rust 1.0.7 defaults to API 35. Respect Flutter's
      // actual minimum API so the shared library loads on older supported OSes.
      final (rustTarget, clangTarget) = switch (code.targetArchitecture) {
        Architecture.arm64 => (
          'aarch64-linux-android',
          'aarch64-linux-android',
        ),
        Architecture.arm => (
          'armv7-linux-androideabi',
          'armv7a-linux-androideabi',
        ),
        Architecture.x64 => ('x86_64-linux-android', 'x86_64-linux-android'),
        _ => throw UnsupportedError('Unsupported Android architecture'),
      };
      final suffix = Platform.isWindows ? '.cmd' : '';
      final linker = code.cCompiler!.compiler
          .resolve('$clangTarget${code.android.targetNdkApi}-clang$suffix')
          .toFilePath();
      if (!File(linker).existsSync()) {
        throw StateError('Missing NDK linker: $linker');
      }
      environment['CARGO_TARGET_${rustTarget.replaceAll('-', '_').toUpperCase()}_LINKER'] =
          linker;
    }
    await RustBuilder(
      assetName: 'native_bindings.dart',
      cratePath: '../core',
      extraCargoBuildArgs: ['--locked', '--lib'],
      extraCargoEnvironmentVariables: environment,
    ).run(input: input, output: output);
  });
}
