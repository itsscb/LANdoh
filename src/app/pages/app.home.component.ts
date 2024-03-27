// import { confirm } from '@tauri-apps/api/dialog';
// import { homeDir } from '@tauri-apps/api/path';
// import { MessageService } from 'primeng/api';

import { Component, OnInit } from '@angular/core';

import { TableModule } from 'primeng/table';
import { AccordionModule } from 'primeng/accordion';


import { invoke } from '@tauri-apps/api';
import { open } from '@tauri-apps/api/dialog';
import { emit, listen } from '@tauri-apps/api/event'
import { appWindow } from '@tauri-apps/api/window';

import { SharedDirectory } from '../models/source';
import { Directory } from '../models/directory';
import { App } from '../models/app';

@Component({
  selector: 'app-home',
  templateUrl: './app.home.component.html',
  styleUrls: ['./app.home.component.css'],
  // providers: [MessageService]
})
export class AppHomeComponent implements OnInit {
  // constructor(private toastService: MessageService) {}

  app_state() {
    invoke('app_state').then((s) => {
      console.log('raw:',s);
      this.apps = [s as App];
      this.app = structuredClone(s as App);
      // this.app.shared_directories = this.app.shared_directories as SharedDirectory[];
      console.log(this.app);
    })
  }

  update_nickname(nick: string) {
    invoke('update_nickname', {nickname: nick, window: appWindow}).then(() => this.app_state());
  }

  async update_destination() {
    const selected = await open({
      multiple: false,
      directory: true,
    })
    invoke('update_destination', {destination: selected, window: appWindow}).then(() => this.app_state());
  }

  async add_shared_dir() {
    const selected = await open({
      multiple: false,
      directory: true,
    })

    let exists = false;
    this.app.shared_directories.filter((d) => {
      d.paths.filter((p) => {
        if (p == selected) {
          exists = true;
        }
    })});
    
    if (!exists) {
      invoke('add_shared_dir', {path: selected, window: appWindow}).then(() => this.app_state());
    }
  }

  async remove_shared_dir(name: string) {
    invoke('remove_shared_dir', {path: name, window: appWindow}).then(() => this.app_state());
  }


  listen_for() {
    if (!this.listening) {
      console.log("clicked: listen()");
      invoke('listen_for', { window: appWindow });
      this.listening = true;
    }
  }

  serve() {
    if (!this.serving) {
      console.log("clicked: serve()");
      invoke('serve');
      this.serving = true;
    }
  }

  broadcast() {
    if(!this.broadcasting) {
      console.log("broadcasting");
      invoke("broadcast");
      this.broadcasting = true;
    }
  }

  request_dir(id: string, name: string) {
    console.log("clicked request_dir:", id, name);
    invoke("request_dir", { id: id, dir: name });
  }

 async watch() {
    await listen('sources', (event) => {
      this.new_sources = [...event.payload as Directory[]];
      this.app_state();
      // this.toastService.add({
      //   severity: 'success', summary: 'Some new Directories were shared',
      // });
    })
  }

  apps: App[];
  app: App;

  dark: boolean;

  listening = false;
  serving = false;
  broadcasting = false;

  update_sources() {
    this.sources = [...this.new_sources];
    this.new_sources = null;
  }

  sources: Directory[];
  new_sources: Directory[];

  async ngOnInit(): Promise<void> {
   
    // const confirmed = await confirm('Are you sure?', 'Tauri');
    
    this.app_state();
    this.listen_for();
    this.serve();
    this.broadcast();
    await this.watch()
  }


}
