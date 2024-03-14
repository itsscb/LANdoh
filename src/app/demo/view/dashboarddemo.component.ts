import {Component, OnInit} from '@angular/core';
import {SelectItem} from 'primeng/api';
import {BreadcrumbService} from '../../breadcrumb.service';
import {ProductService} from '../service/productservice';
import {Product} from '../domain/product';
import {AppConfig} from '../domain/appconfig';
import {ConfigService} from '../service/app.config.service';
import {Subscription} from 'rxjs';

@Component({
    templateUrl: './dashboard.component.html',
    styleUrls: ['../../../assets/demo/badges.scss']
})
export class DashboardDemoComponent implements OnInit {

    lineChartData: any;

    lineChartOptions: any;

    dropdownYears: SelectItem[];

    selectedYear: any;

    activeNews = 1;

    cars: any[];

    selectedCar: any;

    products: Product[];

    events: any[];

    subscription: Subscription;

    config: AppConfig;

    constructor(private productService: ProductService, private breadcrumbService: BreadcrumbService, public configService: ConfigService) {
      this.breadcrumbService.setItems([
          {label: 'Dashboard', routerLink: ['/']}
      ]); 
    

      this.config = this.configService.config;
      this.subscription = this.configService.configUpdate$.subscribe(config => {
          this.config = config;
          this.updateChartOptions();
      });
    }

    ngOnInit() {
        this.productService.getProducts().then(data => this.products = data);

        this.lineChartData = {
            labels: ['January', 'February', 'March', 'April', 'May', 'June', 'July'],
            datasets: [
                {
                    label: 'Sapphire',
                    data: [1, 2, 5, 3, 12, 7, 15],
                    borderColor: [
                        '#45b0d5'
                    ],
                    borderWidth: 3,
                    fill: false,
                    tension: .4
                },
                {
                    label: 'Roma',
                    data: [3, 7, 2, 17, 15, 13, 19],
                    borderColor: [
                        '#d08770'
                    ],
                    borderWidth: 3,
                    fill: false,
                    tension: .4
                }
            ]
        };
        this.lineChartOptions = {
            responsive: true,
            maintainAspectRatio: true,
            fontFamily: '\'Candara\', \'Calibri\', \'Courier\', \'serif\'',
            hover: {
                mode: 'index'
            },
            scales: {
                x: {
                    grid: {
                        display: false
                    },
                    ticks: {
                        color: '#9199a9'
                    }
                },
                y: {
                    grid: {
                        display: false
                    },
                    ticks: {
                        color: '#9199a9'
                    }
                }
            },
            plugins: {
                legend: {
                    display: true,
                    labels: {
                        color: '#9199a9'
                    }
                }
            }
        };

        this.dropdownYears = [
            {label: '2019', value: 2019},
            {label: '2018', value: 2018},
            {label: '2017', value: 2017},
            {label: '2016', value: 2016},
            {label: '2015', value: 2015},
            {label: '2014', value: 2014}
        ];
    }
    updateChartOptions() {
        if (this.config.dark)
            this.applyDarkTheme();
        else
            this.applyLightTheme();

    }

    applyDarkTheme() {
        this.lineChartOptions = {
            plugins: {
                legend: {
                    labels: {
                        color: '#ebedef'
                    }
                }
            },
            scales: {
                x: {
                    ticks: {
                        color: '#ebedef'
                    },
                    grid: {
                        display: false
                    }
                },
                y: {
                    ticks: {
                        color: '#ebedef'
                    },
                    grid: {
                        display: false
                    }
                },
            }
        };
    }

    applyLightTheme() {
            this.lineChartOptions = {
            plugins: {
                legend: {
                    labels: {
                        color: '#495057'
                    }
                }
            },
            scales: {
                x: {
                    ticks: {
                        color: '#495057'
                    },
                    grid: {
                        display: false
                    }
                },
                y: {
                    ticks: {
                        color: '#495057'
                    },
                    grid: {
                        display: false
                    }
                },
            }
        };
    }

    ngOnDestroy() {
        this.subscription.unsubscribe();
    }
}
