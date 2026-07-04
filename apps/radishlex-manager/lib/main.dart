import 'package:flutter/widgets.dart';

import 'src/bridge/manager_bridge_factory.dart';
import 'src/app.dart';

void main() {
  runApp(RadishLexManagerApp(bridge: createDefaultManagerBridge()));
}
