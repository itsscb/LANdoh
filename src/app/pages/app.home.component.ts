import { Component, OnInit } from '@angular/core';

import { TableModule } from 'primeng/table';

import { Source } from '../models/source';

import { invoke } from '@tauri-apps/api';
import { emit, listen } from '@tauri-apps/api/event'
import { appWindow } from '@tauri-apps/api/window';
import { Directory } from '../models/directory';

@Component({
  selector: 'app-home',
  templateUrl: './app.home.component.html',
  styleUrls: ['./app.home.component.css'],
})
export class AppHomeComponent implements OnInit {

  // test_emit() {
  //   console.log("clicked: test_emit()");
  //   invoke('test_emit', { window: appWindow });
  // }

  listen() {
    console.log("clicked: listen()");
    invoke('listen', { window: appWindow });
    this.listening = true;
  }

  serve() {
    console.log("clicked: serve()");
    invoke('serve');
    this.serving = true;
  }


  request_dir(id: string, name: string) {
    console.log("clicked: request_dir(", id, name, ")");
    invoke("request_dir", { id: id, dir: name });
  }

  dark: boolean;

  checked: boolean;

  listening = false;
  serving = false;

  sources: Directory[];

  async ngOnInit(): Promise<void> {
    // const unlisten_emit = await listen('test_emit', (event) => {
    //   // event.event is the event name (useful if you want to use a single callback fn for multiple event types)
    //   // event.payload is the payload object
    //   let s = event.payload as Source;
    //   this.sources = [...this.sources, s];
    //   console.log(this.sources);
    //   this.checked = !this.checked;
    // })
    const unlisten_listen = await listen('sources', (event) => {
      // event.event is the event name (useful if you want to use a single callback fn for multiple event types)
      // event.payload is the payload object
      this.sources = [...event.payload as Directory[]];
      console.log(event.payload);
    })
    this.sources = [{
      id: "1",
      nickname: "nick-1",
      name: "test",
      ip: "localhost"
    }
    ];
  }


}
