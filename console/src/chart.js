export function register_plugin() {
    Chart.register({
        id: "donutcenter",
        afterUpdate: function (chart) {
            if (chart.config.options.elements.center) {
                var helpers = Chart.helpers;
                var centerConfig = chart.config.options.elements.center;
                var globalConfig = Chart.defaults;
                var ctx = chart.ctx;

                var fontStyle = helpers.valueOrDefault(centerConfig.fontStyle, globalConfig.defaultFontStyle);
                var fontFamily = helpers.valueOrDefault(centerConfig.fontFamily, globalConfig.defaultFontFamily);

                if (centerConfig.fontSize)
                    var fontSize = centerConfig.fontSize;
                    // figure out the best font size, if one is not specified
                else {
                    ctx.save();
                    var fontSize = helpers.valueOrDefault(centerConfig.minFontSize, 1);
                    var maxFontSize = helpers.valueOrDefault(centerConfig.maxFontSize, 256);
                    var maxText = helpers.valueOrDefault(centerConfig.maxText, centerConfig.text);

                    const innerRadius = chart._metasets[chart._metasets.length-1].data[0].innerRadius;
                    do {
                        ctx.font = helpers.fontString(fontSize, fontStyle, fontFamily);
                        var textWidth = ctx.measureText(maxText).width;

                        // check if it fits, is within configured limits and that we are not simply toggling back and forth
                        if (textWidth < innerRadius * 2 && fontSize < maxFontSize)
                            fontSize += 1;
                        else {
                            // reverse last step
                            fontSize -= 1;
                            break;
                        }
                    } while (true)
                    ctx.restore();
                }

                // save properties
                chart.center = {
                    font: helpers.fontString(fontSize, fontStyle, fontFamily),
                    fillStyle: helpers.valueOrDefault(centerConfig.fontColor, globalConfig.defaultFontColor)
                };
            }
        },
        afterDraw: function (chart) {
            if (chart.center) {
                var centerConfig = chart.config.options.elements.center;
                var ctx = chart.ctx;

                ctx.save();
                ctx.font = chart.center.font;
                ctx.fillStyle = chart.center.fillStyle;
                ctx.textAlign = 'center';
                ctx.textBaseline = 'middle';
                var centerX = (chart.chartArea.left + chart.chartArea.right) / 2;
                var centerY = (chart.chartArea.top + chart.chartArea.bottom) / 2;
                ctx.fillText(centerConfig.text, centerX, centerY);
                ctx.restore();
            }
        },
    })
}

export function gauge_chart(props, is_update) {
    console.log("ID: ", JSON.stringify(props));
    const label = props.label;
    var data = [];
    var colors = [];
    var labels = [];
    for (var key in props.values) {
        data.push(props.values[key][0]);
        colors.push(props.values[key][1]);
        if (props.values[key][2] != null) {
            labels.push(props.values[key][2]);
        }
    }
    const mydata = {
        data: data,
        backgroundColor: colors,
        hoverOffset: 0
    };

    var config = {
        type: 'doughnut',
        data: {
            datasets: [
                mydata,
            ]
        },
        options: {
            cutout: '75%',
            responsive: true,
            plugins: {
                legend: {
                    position: 'bottom'
                },
            },
            elements: {
                arc: {
                    roundedCornersFor: 0
                }
            },
            animation: {
                duration: 0,
            },
        },

    };

    if (labels.length > 0) {
        config.data.labels = labels;
    }

    const title = props.title;
    if (title != null) {
        config.options.plugins.title = {
            display: true,
            text: title,
            font: {
                size: 42,
            }
        };
    }

    if (label != null) {
        config.options.elements.center = {
            maxText: '   100%',
            text: label,
            fontColor: "black",
            fontFamily: "'Helvetica Neue', 'Helvetica', 'Arial', sans-serif",
            fontStyle: 'bold',
            // fontSize: 12,
            // if a fontSize is NOT specified, we will scale (within the below limits) maxText to take up the maximum space in the center
            // if these are not specified either, we default to 1 and 256
            minFontSize: 1,
            maxFontSize: 256,
        };
    }

    const element = document.getElementById(props.id);
    if (element != null) {
        if (Chart.getChart(props.id) === undefined) {
            console.log("Creating chart with id " + JSON.stringify(props.id));
            const myChart = new Chart(
                element,
                config);
        } else {
            console.log("Updating chart with id " + JSON.stringify(props.id));
            if (!is_update) {
                const oldChart = Chart.getChart(props.id);
                oldChart.destroy();
                const myChart = new Chart(
                    element,
                    config);
            } else {
                const myChart = Chargs.getChart(props.id);
                myChart.data.datasets.forEach((dataset) => {
                    dataset = [mydata];
                });
                myChart.update();
            }
        }
    }
}
