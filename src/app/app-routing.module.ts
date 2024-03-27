import { RouterModule } from '@angular/router';
import { NgModule } from '@angular/core';

import { AppMainComponent } from './app.main.component';
import { AppHomeComponent } from './pages/app.home.component';

@NgModule({
    imports: [
        RouterModule.forRoot([
            {
                path: '', component: AppMainComponent,
                children: [
                    { path: '', component: AppHomeComponent },
                ]
            },
            { path: '**', redirectTo: '' },
        ], { scrollPositionRestoration: 'enabled' })
    ],
    exports: [RouterModule]
})
export class AppRoutingModule {
}
