#!/usr/bin/env node

import { getLocation, collectPhpClasses } from './php.js';
import { getAtPosition } from './xml.js';

import {
  createConnection,
  InitializeParams,
  InitializeResult,
  ProposedFeatures,
  Location,
  Definition,
} from "vscode-languageserver/node.js";

let connection = createConnection(ProposedFeatures.all);

connection.onInitialize((params: InitializeParams) => {
  if (params.workspaceFolders) {
    params.workspaceFolders.forEach((folder) => {
      let path = folder.uri.slice(7); // get rid of file://
      collectPhpClasses(path);
    });
  }

  const result: InitializeResult = {
    capabilities: {
      definitionProvider: true,
      declarationProvider: true,
    },
  };
  return result;
});

interface Settings { }

const defaultSettings: Settings = {};
let globalSettings: Settings = defaultSettings;


connection.onDidChangeConfiguration((change) => {
  globalSettings = <Settings>change.settings;
});

connection.onDefinition((params): Definition | null => {
  let token = getAtPosition(params.textDocument.uri, params.position);
  if (token == null) {
    return null;
  }
  let location;
  if (typeof token == "string") {
    location = getLocation(token);
  } else {
    location = getLocation(token.class_name, token.method_name);
  }

  if (location == null) {
    return null;
  }

  return Location.create('file://' + location.file, {
    start: location.start,
    end: location.end
  });
});

connection.listen();
