import 'package:flutter/widgets.dart';

import 'src/bridge/manager_bridge_factory.dart';
import 'src/app.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final bootstrap = await createDefaultManagerBootstrap();
  runApp(
    RadishLexManagerApp(bridge: bootstrap.bridge, runtimeMode: bootstrap.mode),
  );
}
