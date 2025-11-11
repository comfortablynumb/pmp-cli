/**
 * Minimal jQuery-compatible API for PMP UI
 * Provides basic DOM manipulation, AJAX, and event handling
 */
(function(window) {
    'use strict';

    class PmpQuery {
        constructor(selector) {
            if (typeof selector === 'string') {
                // Check if it's HTML (starts with '<')
                if (selector.trim().startsWith('<')) {
                    // Create element from HTML string
                    const template = document.createElement('template');
                    template.innerHTML = selector.trim();
                    this.elements = Array.from(template.content.children);
                } else {
                    // It's a CSS selector
                    try {
                        this.elements = Array.from(document.querySelectorAll(selector));
                    } catch (e) {
                        console.error('Invalid selector:', selector, e);
                        this.elements = [];
                    }
                }
            } else if (selector instanceof Element) {
                this.elements = [selector];
            } else if (selector instanceof NodeList || Array.isArray(selector)) {
                this.elements = Array.from(selector);
            } else if (selector === document || selector === window) {
                this.elements = [selector];
            } else {
                this.elements = [];
            }
            this.length = this.elements.length;
        }

        each(callback) {
            this.elements.forEach((el, i) => callback.call(el, i, el));
            return this;
        }

        on(event, handler) {
            return this.each(function() {
                this.addEventListener(event, handler);
            });
        }

        off(event, handler) {
            return this.each(function() {
                this.removeEventListener(event, handler);
            });
        }

        addClass(className) {
            return this.each(function() {
                this.classList.add(...className.split(' '));
            });
        }

        removeClass(className) {
            return this.each(function() {
                this.classList.remove(...className.split(' '));
            });
        }

        toggleClass(className) {
            return this.each(function() {
                this.classList.toggle(className);
            });
        }

        hasClass(className) {
            return this.elements.length > 0 && this.elements[0].classList.contains(className);
        }

        attr(name, value) {
            if (value === undefined) {
                return this.elements.length > 0 ? this.elements[0].getAttribute(name) : null;
            }
            return this.each(function() {
                this.setAttribute(name, value);
            });
        }

        removeAttr(name) {
            return this.each(function() {
                this.removeAttribute(name);
            });
        }

        prop(name, value) {
            if (value === undefined) {
                return this.elements.length > 0 ? this.elements[0][name] : null;
            }
            return this.each(function() {
                this[name] = value;
            });
        }

        val(value) {
            if (value === undefined) {
                return this.elements.length > 0 ? this.elements[0].value : null;
            }
            return this.each(function() {
                this.value = value;
            });
        }

        text(content) {
            if (content === undefined) {
                return this.elements.length > 0 ? this.elements[0].textContent : '';
            }
            return this.each(function() {
                this.textContent = content;
            });
        }

        html(content) {
            if (content === undefined) {
                return this.elements.length > 0 ? this.elements[0].innerHTML : '';
            }
            return this.each(function() {
                this.innerHTML = content;
            });
        }

        append(content) {
            return this.each(function() {
                if (typeof content === 'string') {
                    this.insertAdjacentHTML('beforeend', content);
                } else if (content instanceof Element) {
                    this.appendChild(content);
                } else if (content instanceof PmpQuery) {
                    content.elements.forEach(el => this.appendChild(el.cloneNode(true)));
                }
            });
        }

        prepend(content) {
            return this.each(function() {
                if (typeof content === 'string') {
                    this.insertAdjacentHTML('afterbegin', content);
                } else if (content instanceof Element) {
                    this.insertBefore(content, this.firstChild);
                }
            });
        }

        empty() {
            return this.each(function() {
                this.innerHTML = '';
            });
        }

        remove() {
            return this.each(function() {
                this.parentNode && this.parentNode.removeChild(this);
            });
        }

        find(selector) {
            const found = [];
            this.elements.forEach(el => {
                found.push(...el.querySelectorAll(selector));
            });
            return new PmpQuery(found);
        }

        parent() {
            const parents = this.elements.map(el => el.parentElement).filter(Boolean);
            return new PmpQuery(parents);
        }

        children() {
            const children = [];
            this.elements.forEach(el => children.push(...el.children));
            return new PmpQuery(children);
        }

        data(key, value) {
            if (value === undefined) {
                return this.elements.length > 0 ? this.elements[0].dataset[key] : null;
            }
            return this.each(function() {
                this.dataset[key] = value;
            });
        }

        is(selector) {
            return this.elements.length > 0 && this.elements[0].matches(selector);
        }

        css(prop, value) {
            if (typeof prop === 'object') {
                return this.each(function() {
                    Object.assign(this.style, prop);
                });
            }
            if (value === undefined) {
                return this.elements.length > 0 ?
                    window.getComputedStyle(this.elements[0])[prop] : null;
            }
            return this.each(function() {
                this.style[prop] = value;
            });
        }

        show() {
            return this.each(function() {
                this.style.display = '';
            });
        }

        hide() {
            return this.each(function() {
                this.style.display = 'none';
            });
        }

        toggle() {
            return this.each(function() {
                this.style.display = this.style.display === 'none' ? '' : 'none';
            });
        }

        first() {
            return new PmpQuery(this.elements[0] || []);
        }

        last() {
            return new PmpQuery(this.elements[this.elements.length - 1] || []);
        }

        eq(index) {
            return new PmpQuery(this.elements[index] || []);
        }

        get(index) {
            return index === undefined ? this.elements : this.elements[index];
        }
    }

    // Main $ function
    function $(selector) {
        if (typeof selector === 'function') {
            // Document ready
            if (document.readyState === 'loading') {
                document.addEventListener('DOMContentLoaded', selector);
            } else {
                selector();
            }
            return;
        }
        return new PmpQuery(selector);
    }

    // AJAX methods
    $.ajax = function(options) {
        return new Promise((resolve, reject) => {
            const xhr = new XMLHttpRequest();
            const method = (options.method || options.type || 'GET').toUpperCase();
            const url = options.url;
            const data = options.data;
            const contentType = options.contentType || 'application/x-www-form-urlencoded';

            xhr.open(method, url, true);

            // Set headers
            if (options.headers) {
                Object.keys(options.headers).forEach(key => {
                    xhr.setRequestHeader(key, options.headers[key]);
                });
            }

            if (contentType && method !== 'GET') {
                xhr.setRequestHeader('Content-Type', contentType);
            }

            // Handle response
            xhr.onload = function() {
                if (xhr.status >= 200 && xhr.status < 300) {
                    let response = xhr.responseText;
                    if (xhr.getResponseHeader('Content-Type')?.includes('application/json')) {
                        try {
                            response = JSON.parse(response);
                        } catch (e) {
                            // Keep as text
                        }
                    }
                    resolve(response);
                } else {
                    const error = new Error(xhr.statusText || 'Request failed');
                    error.status = xhr.status;
                    error.responseText = xhr.responseText;
                    try {
                        error.responseJSON = JSON.parse(xhr.responseText);
                    } catch (e) {
                        // No JSON response
                    }
                    reject(error);
                }
            };

            xhr.onerror = function() {
                const error = new Error('Network error');
                error.status = 0;
                reject(error);
            };

            // Send request
            if (method === 'GET' || !data) {
                xhr.send();
            } else if (typeof data === 'string') {
                xhr.send(data);
            } else if (contentType === 'application/json') {
                xhr.send(JSON.stringify(data));
            } else {
                xhr.send(data);
            }
        });
    };

    $.get = function(url, data, success) {
        const queryString = data ? '?' + new URLSearchParams(data).toString() : '';
        return $.ajax({
            url: url + queryString,
            method: 'GET'
        }).then(response => {
            if (success) success(response);
            return response;
        });
    };

    $.post = function(url, data, success) {
        return $.ajax({
            url: url,
            method: 'POST',
            data: data,
            contentType: 'application/json'
        }).then(response => {
            if (success) success(response);
            return response;
        });
    };

    $.getJSON = function(url, data, success) {
        return $.get(url, data, success);
    };

    // Utility methods
    $.extend = function(target, ...sources) {
        sources.forEach(source => {
            Object.keys(source).forEach(key => {
                target[key] = source[key];
            });
        });
        return target;
    };

    $.each = function(obj, callback) {
        if (Array.isArray(obj)) {
            obj.forEach((val, i) => callback(i, val));
        } else {
            Object.keys(obj).forEach(key => callback(key, obj[key]));
        }
    };

    $.map = function(arr, callback) {
        return arr.map((val, i) => callback(val, i));
    };

    $.param = function(obj) {
        return new URLSearchParams(obj).toString();
    };

    // Export to window
    window.$ = window.jQuery = $;

})(window);
